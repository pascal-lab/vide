//! Read-only loader for FuseSoC CAPI2 `.core` files.
//!
//! This crate parses FuseSoC CAPI2 core files into a neutral project model
//! suitable for IDE consumption.  It deliberately does NOT implement:
//!
//! - provider fetch (git/github/svn/opencores downloads)
//! - generator execution
//! - build/export materialization (Edalize/EDAM)
//! - hooks/scripts
//! - global `fusesoc.conf` library management
//! - remote dependency resolution / SAT solving
//!
//! What it does implement:
//!
//! 1. `CAPI=2:` preamble stripping
//! 2. YAML deserialization into a typed [`raw`] model
//! 3. CAPI2 conditional expression parsing and evaluation ([`expr`])
//! 4. YAML merge key (`<<`) resolution via `serde_yaml_ng::Value::apply_merge`
//! 5. `*_append` normalization and file attribute inheritance ([`normalize`])
//! 6. VLNV parsing and version relations ([`vlnv`])
//! 7. Local-only dependency resolution ([`resolve`])
//! 8. Target/fileset expansion into [`ResolvedProject`] ([`project`])
//!
//! The output [`ResolvedProject`] is a flat, tool-agnostic description of
//! source files, include directories, defines, and top-level modules.

pub mod expr;
pub mod normalize;
pub mod project;
pub mod raw;
pub mod resolve;
pub mod vlnv;

pub use project::{ResolvedCore, ResolvedFile, ResolvedProject};
pub use raw::{
    Core, FileAttributes, FileEntry, Fileset, Parameter, Provider, ProviderKind, Target,
};
pub use vlnv::{VersionRelation, Vlnv, VlnvRequirement};

/// Errors produced while loading a `.core` file.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("missing CAPI=2 preamble on first line")]
    MissingPreamble,
    #[error("unsupported CAPI version: {0}")]
    UnsupportedVersion(String),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("missing required field `{field}`")]
    MissingField { field: String },
    #[error("unsupported feature `{feature}` in {context}: {detail}")]
    Unsupported { feature: String, context: String, detail: String },
    #[error("dependency resolution failed: {0}")]
    Resolution(String),
}

/// Read a `.core` file from disk, strip the preamble, parse YAML, and return
/// the raw [`Core`] model.
pub fn load_core_file(path: &utils::paths::AbsPathBuf) -> Result<raw::Core, CoreError> {
    let text = std::fs::read_to_string(path.as_path()).map_err(|e| CoreError::Io(e.to_string()))?;
    let stripped = strip_preamble(&text)?;
    // `serde_yaml_ng` does NOT resolve YAML merge keys (`<<`) during
    // deserialization; `apply_merge` must be called explicitly.  Without it,
    // `<<` reaches the typed model and trips `deny_unknown_fields`.
    let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(stripped)?;
    value.apply_merge()?;
    let core: raw::Core = serde_yaml_ng::from_value(value)?;
    Ok(core)
}

/// Strip the `CAPI=2:` preamble from the first line.
///
/// FuseSoC requires the first line to be exactly `CAPI=2:` (possibly with
/// surrounding whitespace).  Lines before it are not allowed; lines after it
/// form the YAML body.
pub fn strip_preamble(text: &str) -> Result<&str, CoreError> {
    let mut lines = text.lines();
    let first = lines.next().ok_or(CoreError::MissingPreamble)?;
    let trimmed = first.trim();
    if trimmed == "CAPI=2:" {
        // Return the rest of the text after the first line.
        let offset = first.len()
            + text[first.len()..].chars().take_while(|c| *c == '\n' || *c == '\r').count();
        Ok(&text[offset..])
    } else if trimmed.starts_with("CAPI=") {
        let version = trimmed.strip_prefix("CAPI=").unwrap_or("").trim_end_matches(':');
        Err(CoreError::UnsupportedVersion(version.to_owned()))
    } else {
        Err(CoreError::MissingPreamble)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_preamble() {
        let text = "CAPI=2:\nname: test\n";
        assert_eq!(strip_preamble(text).unwrap(), "name: test\n");
    }

    #[test]
    fn rejects_missing_preamble() {
        let text = "name: test\n";
        assert!(strip_preamble(text).is_err());
    }

    #[test]
    fn rejects_capi1() {
        let text = "CAPI=1:\nname: test\n";
        assert!(strip_preamble(text).is_err());
    }

    #[test]
    fn handles_empty_body() {
        let text = "CAPI=2:\n";
        assert_eq!(strip_preamble(text).unwrap(), "");
    }

    #[test]
    fn resolves_yaml_merge_keys() {
        let text = "\
CAPI=2:
name: test:core:1.0.0
filesets:
  rtl:
    files: [a.v]
    file_type: verilogSource
targets:
  default: &default
    filesets: [rtl]
    toplevel: top
  sim:
    <<: *default
    description: simulate
    filesets_append: [tb]
    tools:
      icarus:
        iverilog_options: [-g2012]
";
        let core =
            load_core_file(&utils::paths::AbsPathBuf::assert(write_core_to_temp(text))).unwrap();
        let sim = &core.targets["sim"];
        // Merge key pulled in the anchor's fields.
        assert_eq!(sim.filesets, vec!["rtl"]);
        assert_eq!(sim.toplevel, vec!["top"]);
        // Child fields still present.
        assert_eq!(sim.description, "simulate");
        assert_eq!(sim.filesets_append, vec!["tb"]);
        // Opaque tools config preserved.
        assert!(sim.tools.as_ref().unwrap().contains_key("icarus"));
    }

    fn write_core_to_temp(text: &str) -> utils::paths::Utf8PathBuf {
        let dir = std::env::temp_dir().join(format!("vide-fusesoc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.core");
        std::fs::write(&path, text).unwrap();
        utils::paths::Utf8PathBuf::from_path_buf(path).unwrap()
    }
}
