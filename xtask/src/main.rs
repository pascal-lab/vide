use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use anyhow::{Context, bail, ensure};

fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("install") => {
            ensure!(args.next().is_none(), "unexpected arguments for `cargo xtask install`");
            install()
        }
        Some(command) => bail!("unknown xtask command: {command}"),
        None => bail!("usage: cargo xtask install"),
    }
}

fn install() -> anyhow::Result<()> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("failed to resolve repository root")?;
    let vscode_dir = repo_root.join("editors").join("vscode");
    let target = host_package_target()?;
    let vsix_path = vscode_dir.join(format!("vizsla-vscode-{target}.vsix"));
    let vsix_path = vsix_path.to_str().context("VSIX path is not valid UTF-8")?;

    let commit_hash =
        capture("git", &["rev-parse", "--short=12", "HEAD"], &repo_root)?.trim().to_owned();
    ensure!(!commit_hash.is_empty(), "git returned an empty commit hash");

    let build_date = capture("date", &["-u", "+%F"], &repo_root)?.trim().to_owned();
    ensure!(!build_date.is_empty(), "date returned an empty build date");

    let npm_script = format!("package:{target}");
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    run(
        &npm,
        &["run", &npm_script],
        &vscode_dir,
        &[("VIZSLA_COMMIT_HASH", commit_hash.as_str()), ("VIZSLA_BUILD_DATE", build_date.as_str())],
    )
    .with_context(|| format!("failed to package VS Code extension for {target}"))?;

    run(code_command(), &["--install-extension", vsix_path], &repo_root, &[])
        .with_context(|| format!("failed to install VSIX {vsix_path}"))?;

    println!("installed {vsix_path}");

    Ok(())
}

fn host_package_target() -> anyhow::Result<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("macos", "x86_64") => Ok("darwin-x64"),
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("windows", "x86_64") => Ok("win32-x64"),
        ("windows", "aarch64") => Ok("win32-arm64"),
        (os, arch) => bail!("unsupported host platform: {os}-{arch}"),
    }
}

fn code_command() -> &'static str {
    if cfg!(target_os = "macos")
        && Path::new("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code")
            .exists()
    {
        "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"
    } else if cfg!(windows) {
        "code.cmd"
    } else {
        "code"
    }
}

fn capture(command: &str, args: &[&str], cwd: &Path) -> anyhow::Result<String> {
    let output = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to spawn `{}`", render_command(command, args)))?;

    if !output.status.success() {
        bail!(
            "`{}` failed with status {}",
            render_command(command, args),
            format_status(output.status)
        );
    }

    String::from_utf8(output.stdout)
        .with_context(|| format!("`{}` returned non-UTF-8 output", render_command(command, args)))
}

fn run(command: &str, args: &[&str], cwd: &Path, extra_env: &[(&str, &str)]) -> anyhow::Result<()> {
    let mut child = Command::new(command);
    child.args(args).current_dir(cwd).envs(extra_env.iter().copied());

    let status = child
        .status()
        .with_context(|| format!("failed to spawn `{}`", render_command(command, args)))?;

    if status.success() {
        return Ok(());
    }

    bail!("`{}` failed with status {}", render_command(command, args), format_status(status));
}

fn render_command(command: &str, args: &[&str]) -> String {
    let mut rendered = String::from(command);
    for arg in args {
        rendered.push(' ');
        rendered.push_str(arg);
    }
    rendered
}

fn format_status(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => code.to_string(),
        None => "terminated by signal".to_string(),
    }
}
