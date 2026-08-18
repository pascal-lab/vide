use std::io::{BufReader, BufWriter, Write};
#[cfg(not(test))]
use std::process::{Command, Stdio};

use anyhow::Context;
#[cfg(not(test))]
use anyhow::bail;
use preproc_expand::profile_compiler::{
    ProfileCompilationJob, ProfileCompilationOutput, run_profile_compilation,
};

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

pub(crate) fn compile(job: &ProfileCompilationJob) -> anyhow::Result<ProfileCompilationOutput> {
    #[cfg(test)]
    {
        Ok(run_profile_compilation(job.clone()))
    }

    #[cfg(not(test))]
    {
        let executable = std::env::current_exe().context("failed to locate vide executable")?;
        let mut child = Command::new(executable)
            .arg("--compiler-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to start compiler worker")?;
        {
            let mut stdin = child.stdin.take().expect("piped compiler stdin must exist");
            serde_json::to_writer(&mut stdin, job).context("failed to encode compiler job")?;
            stdin.flush().context("failed to flush compiler job")?;
        }
        let output = child.wait_with_output().context("failed to wait for compiler worker")?;
        if !output.status.success() {
            bail!(
                "compiler worker exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        serde_json::from_slice(&output.stdout).context("invalid compiler worker result")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn malformed_job_fails_before_compilation() {
        let error = super::run("not json".as_bytes(), Vec::new()).unwrap_err();
        assert!(error.to_string().contains("invalid compiler job"));
    }
}
