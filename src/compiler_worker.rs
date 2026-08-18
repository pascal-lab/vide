#[cfg(not(test))]
use std::process::{Command, Stdio};
use std::{
    io::{BufReader, BufWriter, Write},
    sync::{Condvar, Mutex, OnceLock},
    time::Duration,
};

use anyhow::Context;
#[cfg(not(test))]
use anyhow::bail;
use preproc_expand::profile_compiler::{
    ProfileCompilationJob, ProfileCompilationOutput, run_profile_compilation,
};
use utils::cancellation::CancellationToken;

const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_WORKER_JOBS: usize = 2;

pub fn run_stdio() -> anyhow::Result<()> {
    let input = std::io::stdin();
    let output = std::io::stdout();
    run(BufReader::new(input.lock()), BufWriter::new(output.lock()))
}

fn run(input: impl std::io::Read, mut output: impl Write) -> anyhow::Result<()> {
    let job: ProfileCompilationJob =
        serde_json::from_reader(input).context("invalid compiler job")?;
    let result = run_profile_compilation(job);
    serde_json::to_writer(&mut output, &result).context("failed to encode compiler result")?;
    output.flush().context("failed to flush compiler result")
}

pub(crate) fn compile(
    job: &ProfileCompilationJob,
    cancellation: &CancellationToken,
) -> anyhow::Result<ProfileCompilationOutput> {
    cancellation.check()?;
    let _slot = acquire_worker_slot(cancellation)?;
    compile_with_timeout(job, cancellation, worker_timeout())
}

fn compile_with_timeout(
    job: &ProfileCompilationJob,
    cancellation: &CancellationToken,
    timeout: Duration,
) -> anyhow::Result<ProfileCompilationOutput> {
    #[cfg(test)]
    {
        let _ = timeout;
        cancellation.check()?;
        Ok(run_profile_compilation(job.clone()))
    }

    #[cfg(not(test))]
    {
        compile_in_child(job, cancellation, timeout)
    }
}

#[cfg(not(test))]
fn compile_in_child(
    job: &ProfileCompilationJob,
    cancellation: &CancellationToken,
    timeout: Duration,
) -> anyhow::Result<ProfileCompilationOutput> {
    let executable = std::env::current_exe().context("failed to locate vide executable")?;
    let mut command = Command::new(executable);
    utils::process::configure_process_tree(&mut command);
    let child = command
        .arg("--compiler-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start compiler worker")?;
    let input = serde_json::to_vec(job).context("failed to encode compiler job")?;
    let output = match utils::process::wait_with_input_and_output_and_cancellation_and_timeout(
        child,
        input,
        cancellation,
        timeout,
    ) {
        Ok(output) => output,
        Err(error) if error.is::<utils::cancellation::CancellationError>() => {
            return Err(error);
        }
        Err(error) => {
            if let Some(timeout) = error.downcast_ref::<utils::process::ProcessTimeout>() {
                bail!("{}", timeout_message(job, timeout.timeout, timeout.pid));
            }
            return Err(error);
        }
    };
    if !output.status.success() {
        bail!(
            "compiler worker exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("invalid compiler worker result")
}

fn worker_timeout() -> Duration {
    std::env::var("VIDE_COMPILER_WORKER_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_WORKER_TIMEOUT)
}

fn worker_job_limit() -> usize {
    std::env::var("VIDE_COMPILER_WORKER_JOBS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_WORKER_JOBS)
        .max(1)
}

fn timeout_message(job: &ProfileCompilationJob, timeout: Duration, pid: u32) -> String {
    let bytes: usize = job
        .buffers
        .iter()
        .map(|buffer| buffer.text.as_deref().map(str::len).unwrap_or(0))
        .sum();
    format!(
        "compiler worker timed out after {timeout:?} (pid={pid}, roots={}, buffers={}, bytes={bytes})",
        job.roots.len(),
        job.buffers.len(),
    )
}

struct WorkerLimiter {
    max: usize,
    in_flight: Mutex<usize>,
    ready: Condvar,
}

struct WorkerSlot {
    limiter: &'static WorkerLimiter,
}

fn limiter() -> &'static WorkerLimiter {
    static LIMITER: OnceLock<WorkerLimiter> = OnceLock::new();
    LIMITER.get_or_init(|| WorkerLimiter {
        max: worker_job_limit(),
        in_flight: Mutex::new(0),
        ready: Condvar::new(),
    })
}

fn acquire_worker_slot(cancellation: &CancellationToken) -> anyhow::Result<WorkerSlot> {
    let limiter = limiter();
    let mut in_flight = limiter.in_flight.lock().unwrap_or_else(|poison| poison.into_inner());
    loop {
        cancellation.check()?;
        if *in_flight < limiter.max {
            *in_flight += 1;
            return Ok(WorkerSlot { limiter });
        }
        let (guard, _) = limiter
            .ready
            .wait_timeout(in_flight, Duration::from_millis(50))
            .unwrap_or_else(|poison| poison.into_inner());
        in_flight = guard;
    }
}

impl Drop for WorkerSlot {
    fn drop(&mut self) {
        let mut in_flight =
            self.limiter.in_flight.lock().unwrap_or_else(|poison| poison.into_inner());
        *in_flight = in_flight.saturating_sub(1);
        self.limiter.ready.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use preproc_expand::profile_compiler::{
        ProfileCompilationBuffer, ProfileCompilationRoot, ProfileDiagnosticsOptions,
        ProfileRootKind,
    };

    use super::*;

    #[test]
    fn malformed_job_fails_before_compilation() {
        let error = super::run("not json".as_bytes(), Vec::new()).unwrap_err();
        assert!(error.to_string().contains("invalid compiler job"));
    }

    #[test]
    fn timeout_error_reports_job_scale() {
        let job = ProfileCompilationJob {
            profile_id: 0,
            roots: vec![ProfileCompilationRoot {
                file_id: 0,
                kind: ProfileRootKind::SystemVerilog,
                name: "top.sv".to_owned(),
                path: "/top.sv".to_owned(),
            }],
            buffers: vec![ProfileCompilationBuffer {
                file_id: 0,
                path: "/top.sv".to_owned(),
                text: Some("module top; endmodule\n".to_owned()),
            }],
            top_modules: Vec::new(),
            include_dirs: Vec::new(),
            predefines: Vec::new(),
            diagnostics: ProfileDiagnosticsOptions {
                parse: true,
                semantic: true,
                warnings: None,
                rules: Vec::new(),
            },
        };
        let message = timeout_message(&job, Duration::from_secs(30), 4242);
        assert!(message.contains("pid=4242"), "{message}");
        assert!(message.contains("roots=1"), "{message}");
        assert!(message.contains("buffers=1"), "{message}");
        assert!(
            message.contains(&format!(
                "bytes={}",
                job.buffers[0].text.as_deref().map(str::len).unwrap_or(0)
            )),
            "{message}"
        );
    }

    #[test]
    fn compile_propagates_cancellation_before_work() {
        let job = ProfileCompilationJob {
            profile_id: 0,
            roots: Vec::new(),
            buffers: Vec::new(),
            top_modules: Vec::new(),
            include_dirs: Vec::new(),
            predefines: Vec::new(),
            diagnostics: ProfileDiagnosticsOptions {
                parse: true,
                semantic: true,
                warnings: None,
                rules: Vec::new(),
            },
        };
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let error = compile(&job, &cancellation).unwrap_err();
        assert!(error.to_string().contains("cancelled"), "{error:#}");
    }
}
