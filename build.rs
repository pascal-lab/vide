use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=VIDE_BUILD_METADATA");

    let metadata = env::var("VIDE_BUILD_METADATA")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(|value| format!("+{value}"))
        .unwrap_or_default();
    println!("cargo:rustc-env=VIDE_BUILD_METADATA={metadata}");
}
