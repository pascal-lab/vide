use std::{
    io::{Read, Write},
    process::{Child, Command, ExitStatus, Output},
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result};

use crate::cancellation::{CancellationError, CancellationToken};

pub fn configure_process_tree(command: &mut Command) {
    imp::configure_process_tree(command);
}

pub fn wait_with_cancellation(child: &mut Child, cancel: &CancellationToken) -> Result<ExitStatus> {
    let process_tree = imp::ProcessTree::attach(child);
    loop {
        if cancel.is_cancelled() {
            process_tree.kill(child);
            let _ = child.wait();
            return Err(CancellationError.into());
        }

        if let Some(status) = child.try_wait().context("failed to poll child process")? {
            return Ok(status);
        }

        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(windows)]
mod imp {
    use std::{
        mem,
        os::windows::io::AsRawHandle,
        process::{Child, Command},
        ptr,
    };

    use winapi::{
        shared::minwindef::DWORD,
        um::{
            handleapi::CloseHandle,
            jobapi2::{
                AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
                TerminateJobObject,
            },
            winnt::{
                HANDLE, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation,
            },
        },
    };

    pub(super) fn configure_process_tree(_command: &mut Command) {}

    pub(super) struct ProcessTree {
        job: Option<JobHandle>,
    }

    struct JobHandle(HANDLE);

    impl ProcessTree {
        pub(super) fn attach(child: &Child) -> Self {
            match JobHandle::for_child(child) {
                Some(job) => Self { job: Some(job) },
                None => Self { job: None },
            }
        }

        pub(super) fn kill(&self, child: &mut Child) {
            if let Some(job) = &self.job
                && job.terminate()
            {
                return;
            }
            let _ = child.kill();
        }
    }

    impl JobHandle {
        fn for_child(child: &Child) -> Option<Self> {
            let job = unsafe { CreateJobObjectW(ptr::null_mut(), ptr::null()) };
            if job.is_null() {
                tracing::debug!("failed to create Windows job object for child process");
                return None;
            }

            let mut info = unsafe { mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set_info = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &mut info as *mut _ as *mut _,
                    mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as DWORD,
                )
            };
            if set_info == 0 {
                unsafe {
                    CloseHandle(job);
                }
                tracing::debug!("failed to configure Windows job object for child process");
                return None;
            }

            let assigned =
                unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) };
            if assigned == 0 {
                unsafe {
                    CloseHandle(job);
                }
                tracing::debug!("failed to assign child process to Windows job object");
                return None;
            }

            Some(Self(job))
        }

        fn terminate(&self) -> bool {
            unsafe { TerminateJobObject(self.0, 1) != 0 }
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(unix)]
mod imp {
    use std::{
        os::unix::process::CommandExt,
        process::{Child, Command},
    };

    pub(super) fn configure_process_tree(command: &mut Command) {
        command.process_group(0);
    }

    pub(super) struct ProcessTree;

    impl ProcessTree {
        pub(super) fn attach(_child: &Child) -> Self {
            Self
        }

        pub(super) fn kill(&self, child: &mut Child) {
            let process_group = -(child.id() as libc::pid_t);
            unsafe {
                libc::kill(process_group, libc::SIGKILL);
            }
            let _ = child.kill();
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod imp {
    use std::process::{Child, Command};

    pub(super) fn configure_process_tree(_command: &mut Command) {}

    pub(super) struct ProcessTree;

    impl ProcessTree {
        pub(super) fn attach(_child: &Child) -> Self {
            Self
        }

        pub(super) fn kill(&self, child: &mut Child) {
            let _ = child.kill();
        }
    }
}

pub fn wait_with_output_and_cancellation(
    child: Child,
    cancel: &CancellationToken,
) -> Result<Output> {
    wait_with_stdio_and_cancellation(child, None, cancel)
}

pub fn wait_with_input_and_output_and_cancellation(
    child: Child,
    input: Vec<u8>,
    cancel: &CancellationToken,
) -> Result<Output> {
    wait_with_stdio_and_cancellation(child, Some(input), cancel)
}

fn wait_with_stdio_and_cancellation(
    mut child: Child,
    input: Option<Vec<u8>>,
    cancel: &CancellationToken,
) -> Result<Output> {
    let stdin = input.map(|input| {
        child
            .stdin
            .take()
            .map(|stdin| write_all(stdin, input))
            .context("child process stdin is not piped")
    });
    let stdout = child.stdout.take().map(read_to_end);
    let stderr = child.stderr.take().map(read_to_end);

    let status = wait_with_cancellation(&mut child, cancel);
    let stdin = match stdin.transpose() {
        Ok(handle) => join_input(handle),
        Err(error) => Err(error),
    };
    let stdout = join_output(stdout);
    let stderr = join_output(stderr);
    let status = status?;
    let stdout = stdout?;
    let stderr = stderr?;

    if status.success() {
        stdin?;
    }

    Ok(Output { status, stdout, stderr })
}

fn read_to_end<R>(mut reader: R) -> JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut output = Vec::new();
        reader.read_to_end(&mut output)?;
        Ok(output)
    })
}

fn write_all<W>(mut writer: W, input: Vec<u8>) -> JoinHandle<std::io::Result<()>>
where
    W: Write + Send + 'static,
{
    thread::spawn(move || writer.write_all(&input))
}

fn join_input(handle: Option<JoinHandle<std::io::Result<()>>>) -> Result<()> {
    let Some(handle) = handle else {
        return Ok(());
    };
    handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("input writer panicked")))
        .context("failed to write child process input")
}

fn join_output(handle: Option<JoinHandle<std::io::Result<Vec<u8>>>>) -> Result<Vec<u8>> {
    let Some(handle) = handle else {
        return Ok(Vec::new());
    };

    handle
        .join()
        .unwrap_or_else(|_| Err(std::io::Error::other("output reader panicked")))
        .context("failed to read child process output")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        process::{Command, Stdio},
        thread,
        time::Duration,
    };

    use crate::{
        cancellation::{CancellationError, CancellationToken},
        process::{
            configure_process_tree, wait_with_cancellation,
            wait_with_input_and_output_and_cancellation,
        },
    };

    #[test]
    fn cancellation_kills_child_process() {
        let mut command = sleeper_command();
        configure_process_tree(&mut command);
        let mut child = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sleep command should spawn");
        let token = CancellationToken::new();

        token.cancel();
        let error = wait_with_cancellation(&mut child, &token).unwrap_err();

        assert!(error.is::<CancellationError>(), "{error:#}");
        assert!(child.try_wait().expect("child status should be available").is_some());
    }

    #[test]
    fn cancellation_kills_grandchild_process() {
        let dir = tempfile::tempdir().expect("temporary directory should be created");
        let marker = dir.path().join("grandchild-finished");
        let mut command = grandchild_marker_command(dir.path(), &marker);
        configure_process_tree(&mut command);
        let mut child = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("process tree command should spawn");
        let token = CancellationToken::new();

        thread::sleep(Duration::from_millis(200));
        token.cancel();
        let error = wait_with_cancellation(&mut child, &token).unwrap_err();

        assert!(error.is::<CancellationError>(), "{error:#}");
        thread::sleep(Duration::from_millis(2200));
        assert!(!marker.exists(), "grandchild process escaped cancellation");
    }

    #[test]
    fn cancellation_kills_process_while_input_writer_is_active() {
        let mut command = sleeper_command();
        configure_process_tree(&mut command);
        let child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sleep command should spawn");
        let token = CancellationToken::new();
        token.cancel();

        let error =
            wait_with_input_and_output_and_cancellation(child, vec![0; 1024 * 1024], &token)
                .unwrap_err();

        assert!(error.is::<CancellationError>(), "{error:#}");
    }

    #[cfg(windows)]
    fn sleeper_command() -> Command {
        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"]);
        command
    }

    #[cfg(windows)]
    fn grandchild_marker_command(dir: &Path, marker: &Path) -> Command {
        let grandchild = dir.join("grandchild.ps1");
        let child = dir.join("child.ps1");
        fs::write(
            &grandchild,
            r#"
param([string]$Marker)
Start-Sleep -Milliseconds 1500
Set-Content -LiteralPath $Marker -Value done
"#,
        )
        .expect("grandchild script should be written");
        fs::write(
            &child,
            r#"
param([string]$Grandchild, [string]$Marker)
Start-Process -FilePath powershell -WindowStyle Hidden -ArgumentList @('-NoProfile', '-File', $Grandchild, $Marker)
Start-Sleep -Seconds 30
"#,
        )
        .expect("child script should be written");

        let mut command = Command::new("powershell");
        command.args(["-NoProfile", "-File"]).arg(child).arg(grandchild).arg(marker);
        command
    }

    #[cfg(not(windows))]
    fn sleeper_command() -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30"]);
        command
    }

    #[cfg(not(windows))]
    fn grandchild_marker_command(dir: &Path, marker: &Path) -> Command {
        let grandchild = dir.join("grandchild.sh");
        let child = dir.join("child.sh");
        fs::write(&grandchild, "sleep 1.5\nprintf done > \"$1\"\n")
            .expect("grandchild script should be written");
        fs::write(&child, "sh \"$1\" \"$2\" &\nsleep 30\n")
            .expect("child script should be written");

        let mut command = Command::new("sh");
        command.arg(child).arg(grandchild).arg(marker);
        command
    }
}
