use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

const MANIFEST_SCHEMA_PATH: &str = "docs/public/schemas/v1/vizsla.schema.json";

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };

    if args.next().is_some() {
        bail!("unexpected extra arguments");
    }

    match command.as_str() {
        "generate-manifest-schema" => write_manifest_schema(&workspace_root()?),
        "check-manifest-schema" => check_manifest_schema(&workspace_root()?),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        _ => bail!("unknown xtask command: {command}"),
    }
}

fn print_help() {
    eprintln!(
        "Usage: cargo xtask <command>\n\nCommands:\n  generate-manifest-schema\n  check-manifest-schema"
    );
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask manifest directory has no parent")
}

fn manifest_schema_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(MANIFEST_SCHEMA_PATH)
}

fn generated_manifest_schema() -> serde_json::Value {
    project_model::generated_toml_manifest_schema()
}

fn generated_manifest_schema_text() -> Result<String> {
    let generated = generated_manifest_schema();
    Ok(format!("{}\n", serde_json::to_string_pretty(&generated)?))
}

fn write_manifest_schema(workspace_root: &Path) -> Result<()> {
    let schema_path = manifest_schema_path(workspace_root);
    let Some(parent) = schema_path.parent() else {
        bail!("manifest schema path has no parent: {}", schema_path.display());
    };

    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(&schema_path, generated_manifest_schema_text()?)
        .with_context(|| format!("failed to write {}", schema_path.display()))?;
    eprintln!("wrote {}", schema_path.display());
    Ok(())
}

fn check_manifest_schema(workspace_root: &Path) -> Result<()> {
    let schema_path = manifest_schema_path(workspace_root);
    let checked_in: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&schema_path)
            .with_context(|| format!("failed to read {}", schema_path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", schema_path.display()))?;
    let generated = generated_manifest_schema();

    if checked_in != generated {
        bail!("{} is stale; run `cargo xtask generate-manifest-schema`", schema_path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_manifest_schema_matches_generated_schema() {
        check_manifest_schema(&workspace_root().unwrap()).unwrap();
    }
}
