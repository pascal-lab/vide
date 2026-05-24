use std::{env, process::Command};

use time::{OffsetDateTime, macros::format_description};

fn main() {
    println!("cargo:rerun-if-env-changed=VIZSLA_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=VIZSLA_BUILD_DATE");
    watch_git_metadata();

    let release = env::var("PROFILE").as_deref() == Ok("release");
    let commit_hash = build_env("VIZSLA_COMMIT_HASH", release, git_commit_hash);
    let build_date = build_env("VIZSLA_BUILD_DATE", release, utc_build_date);

    println!("cargo:rustc-env=VIZSLA_COMMIT_HASH={commit_hash}");
    println!("cargo:rustc-env=VIZSLA_BUILD_DATE={build_date}");
}

fn build_env(name: &str, release: bool, fallback: impl FnOnce() -> String) -> String {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ if release => fallback(),
        _ => "dev".to_string(),
    }
}

fn git_commit_hash() -> String {
    git_output(["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string())
}

fn watch_git_metadata() {
    for path in [git_path("HEAD"), git_current_branch_path()].into_iter().flatten() {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn git_current_branch_path() -> Option<String> {
    let branch = git_output(["symbolic-ref", "--quiet", "HEAD"])?;
    git_path(&branch)
}

fn git_path(path: impl AsRef<str>) -> Option<String> {
    git_output(["rev-parse", "--git-path", path.as_ref()])
}

fn git_output<const N: usize>(args: [&str; N]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_string())
        .filter(|output| !output.is_empty())
}

fn utc_build_date() -> String {
    format_build_date(OffsetDateTime::now_utc())
}

fn format_build_date(date: OffsetDateTime) -> String {
    let format = format_description!("[year][month][day]T[hour][minute][second]Z");
    date.format(format).expect("UTC build date format should be valid")
}
