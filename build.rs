use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=VIZSLA_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=VIZSLA_BUILD_DATE");

    let release = env::var("PROFILE").as_deref() == Ok("release");
    let commit_hash = require_build_env("VIZSLA_COMMIT_HASH", release);
    let build_date = require_build_env("VIZSLA_BUILD_DATE", release);

    println!("cargo:rustc-env=VIZSLA_COMMIT_HASH={commit_hash}");
    println!("cargo:rustc-env=VIZSLA_BUILD_DATE={build_date}");
}

fn require_build_env(name: &str, release: bool) -> String {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        _ if release => panic!("{name} must be set for release builds"),
        _ => "dev".to_string(),
    }
}
