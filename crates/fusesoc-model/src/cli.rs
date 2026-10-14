//! FuseSoC CLI integration.
//!
//! FuseSoC's supported machine-readable project description is the EDAM YAML
//! emitted by `fusesoc run --setup`. Keep the process boundary here so the
//! rest of the project model consumes the same flat representation regardless
//! of whether the project came from a core file or a VLNV.

use std::{collections::BTreeMap, fs, process::Command};

use saphyr::{LoadableYamlNode, MarkedYaml};
use serde::Deserialize;
use serde_yaml_ng::Value;
use utils::paths::{AbsPath, AbsPathBuf, Utf8PathBuf};

use crate::{ResolvedCore, ResolvedFile, ResolvedProject};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("failed to start FuseSoC CLI: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("FuseSoC CLI failed with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    Failed { status: String, stdout: String, stderr: String },
    #[error("failed to create FuseSoC CLI work directory: {0}")]
    WorkDirectory(#[source] std::io::Error),
    #[error("failed to inspect FuseSoC CLI work directory: {0}")]
    InspectWorkDirectory(#[source] std::io::Error),
    #[error("FuseSoC CLI did not produce an EDAM file in {0}")]
    MissingEdam(AbsPathBuf),
    #[error("FuseSoC CLI produced multiple EDAM files in {0}: {1:?}")]
    MultipleEdam(AbsPathBuf, Vec<AbsPathBuf>),
    #[error("failed to read EDAM file {path}: {source}")]
    ReadEdam { path: AbsPathBuf, source: std::io::Error },
    #[error("failed to parse EDAM file {path}: {source}")]
    ParseEdam { path: AbsPathBuf, source: serde_yaml_ng::Error },
    #[error("EDAM field `{field}` is missing")]
    MissingField { field: &'static str },
    #[error("EDAM field `{field}` has an invalid value: {detail}")]
    InvalidField { field: &'static str, detail: String },
    #[error("invalid FuseSoC core name in {path}: {detail}")]
    CoreName { path: AbsPathBuf, detail: String },
    #[error("failed to read FuseSoC core {path}: {source}")]
    ReadCore { path: AbsPathBuf, source: std::io::Error },
    #[error("failed to parse FuseSoC core in {path}: {detail}")]
    ParseCore { path: AbsPathBuf, detail: String },
}

#[derive(Debug, Deserialize)]
struct Edam {
    #[serde(default)]
    files: Vec<EdamFile>,
    #[serde(default)]
    parameters: BTreeMap<String, EdamParameter>,
    #[serde(default)]
    cores: BTreeMap<String, EdamCore>,
    toplevel: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EdamFile {
    name: String,
    #[serde(default)]
    file_type: String,
    #[serde(default)]
    is_include_file: bool,
    #[serde(default)]
    include_path: Option<String>,
    #[serde(default)]
    logical_name: Option<String>,
    #[serde(default)]
    define: BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct EdamParameter {
    #[serde(default)]
    paramtype: String,
    #[serde(default)]
    default: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct EdamCore {
    core_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreTargetInfo {
    pub name: String,
    pub description: Option<String>,
    pub default_tool: Option<String>,
    pub flow: Option<String>,
    pub has_toplevel: bool,
    #[serde(skip_serializing)]
    pub source_line: u32,
}

/// Load a core file through the FuseSoC CLI.
pub fn load_core(
    core_path: &AbsPathBuf,
    target: &str,
    flags: &[String],
) -> Result<ResolvedProject, CliError> {
    let core_name = read_core_name(core_path)?;
    let workspace_root = core_path.parent().ok_or_else(|| CliError::CoreName {
        path: core_path.clone(),
        detail: "core path has no parent".to_owned(),
    })?;

    load_vlnv(workspace_root, &core_name, target, flags)
}

/// Read only the identity needed to invoke the CLI. FuseSoC remains
/// authoritative for all actual CAPI2 parsing and project expansion.
fn read_core_name(core_path: &AbsPathBuf) -> Result<String, CliError> {
    let text = fs::read_to_string(core_path.as_path())
        .map_err(|source| CliError::ReadCore { path: core_path.clone(), source })?;
    let document = parse_core_document(core_path, &text)?;
    let name =
        document.data.as_mapping_get("name").and_then(|name| name.data.as_str()).ok_or_else(
            || CliError::CoreName {
                path: core_path.clone(),
                detail: "name is missing or not a string".to_owned(),
            },
        )?;
    if name.is_empty() {
        return Err(CliError::CoreName {
            path: core_path.clone(),
            detail: "name is empty".to_owned(),
        });
    }
    Ok(name.to_owned())
}

/// Read the target metadata needed by the project-selection UX.
///
/// This intentionally only reads the target names and display metadata. FuseSoC
/// remains authoritative for dependency resolution and EDAM generation.
pub fn read_core_targets(core_path: &AbsPathBuf) -> Result<Vec<CoreTargetInfo>, CliError> {
    let text = fs::read_to_string(core_path.as_path())
        .map_err(|source| CliError::ReadCore { path: core_path.clone(), source })?;
    read_core_targets_from_text(core_path, &text)
}

/// Read target metadata from an already-loaded core buffer.
pub fn read_core_targets_from_text(
    core_path: &AbsPathBuf,
    text: &str,
) -> Result<Vec<CoreTargetInfo>, CliError> {
    let document = parse_core_document(core_path, text)?;
    let Some(targets) = document.data.as_mapping_get("targets") else {
        return Ok(Vec::new());
    };
    let Some(targets) = targets.data.as_mapping() else {
        return Err(CliError::ParseCore {
            path: core_path.clone(),
            detail: "targets is not a mapping".to_owned(),
        });
    };

    targets
        .iter()
        .map(|(name, target)| {
            let source_line =
                u32::try_from(name.span.start.line()).map_err(|_| CliError::ParseCore {
                    path: core_path.clone(),
                    detail: "target source line is too large".to_owned(),
                })?;
            let name = name.data.as_str().ok_or_else(|| CliError::ParseCore {
                path: core_path.clone(),
                detail: "target name is not a string".to_owned(),
            })?;
            target.data.as_mapping().ok_or_else(|| CliError::ParseCore {
                path: core_path.clone(),
                detail: format!("target `{name}` is not a mapping"),
            })?;
            Ok(CoreTargetInfo {
                name: name.to_owned(),
                description: yaml_string_field(target, "description", core_path)?,
                default_tool: yaml_string_field(target, "default_tool", core_path)?,
                flow: yaml_string_field(target, "flow", core_path)?,
                has_toplevel: target.data.as_mapping_get("toplevel").is_some(),
                source_line,
            })
        })
        .collect()
}

fn yaml_string_field(
    node: &MarkedYaml<'_>,
    field: &'static str,
    core_path: &AbsPathBuf,
) -> Result<Option<String>, CliError> {
    node.data
        .as_mapping_get(field)
        .map(|value| {
            value.data.as_str().map(str::to_owned).ok_or_else(|| CliError::ParseCore {
                path: core_path.clone(),
                detail: format!("field `{field}` is not a string"),
            })
        })
        .transpose()
}

fn parse_core_document<'a>(
    core_path: &AbsPathBuf,
    text: &'a str,
) -> Result<MarkedYaml<'a>, CliError> {
    let body = core_body(core_path, text)?;
    let documents = MarkedYaml::load_from_str(body).map_err(|source| CliError::ParseCore {
        path: core_path.clone(),
        detail: source.to_string(),
    })?;
    let [document] = documents.as_slice() else {
        return Err(CliError::ParseCore {
            path: core_path.clone(),
            detail: format!("expected one YAML document, got {}", documents.len()),
        });
    };
    Ok(document.clone())
}

fn core_body<'a>(core_path: &AbsPathBuf, text: &'a str) -> Result<&'a str, CliError> {
    let (first, body) = text.split_once('\n').ok_or_else(|| CliError::CoreName {
        path: core_path.clone(),
        detail: "missing CAPI=2 preamble".to_owned(),
    })?;
    if first.trim() != "CAPI=2:" {
        return Err(CliError::CoreName {
            path: core_path.clone(),
            detail: format!("expected CAPI=2 preamble, got `{first}`"),
        });
    }
    Ok(body)
}

/// Load a VLNV through the FuseSoC CLI.
pub fn load_vlnv(
    workspace_root: &AbsPath,
    vlnv: &str,
    target: &str,
    flags: &[String],
) -> Result<ResolvedProject, CliError> {
    let work_dir = tempfile::tempdir().map_err(CliError::WorkDirectory)?;
    let work_root = utils::paths::abs_path_buf_from_path_buf(work_dir.path().to_path_buf())
        .ok_or_else(|| CliError::InvalidField {
            field: "work_root",
            detail: format!(
                "temporary path is not an absolute UTF-8 path: {}",
                work_dir.path().display()
            ),
        })?;

    let mut command = Command::new("fusesoc");
    command
        .arg("--monochrome")
        .arg("--cores-root")
        .arg(workspace_root)
        .arg("run")
        .arg("--no-export")
        .arg("--setup")
        .arg("--work-root")
        .arg(work_root.as_path())
        .arg("--target")
        .arg(target);
    for flag in flags {
        command.arg(format!("--flag={flag}"));
    }
    command.arg(vlnv).current_dir(workspace_root);

    tracing::debug!(
        workspace_root = %workspace_root,
        vlnv,
        target,
        flags = ?flags,
        "running FuseSoC CLI to resolve project"
    );

    let output = command.output().map_err(CliError::Spawn)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    tracing::debug!(stdout = %stdout, stderr = %stderr, "FuseSoC CLI completed");
    if !output.status.success() {
        return Err(CliError::Failed {
            status: output
                .status
                .code()
                .map_or_else(|| "terminated by signal".to_owned(), |code| code.to_string()),
            stdout,
            stderr,
        });
    }

    let mut edam_paths = Vec::new();
    for entry in fs::read_dir(work_root.as_path()).map_err(CliError::InspectWorkDirectory)? {
        let path = entry.map_err(CliError::InspectWorkDirectory)?.path();
        if !path.extension().is_some_and(|extension| extension == "yml")
            || !path.file_name().is_some_and(|name| name.to_string_lossy().ends_with(".eda.yml"))
        {
            continue;
        }
        let path = utils::paths::abs_path_buf_from_path_buf(path).ok_or_else(|| {
            CliError::InvalidField {
                field: "EDAM path",
                detail: "EDAM path is not an absolute UTF-8 path".to_owned(),
            }
        })?;
        edam_paths.push(path);
    }

    let edam_path = match edam_paths.as_slice() {
        [] => return Err(CliError::MissingEdam(work_root)),
        [path] => path.clone(),
        paths => return Err(CliError::MultipleEdam(work_root, paths.to_vec())),
    };

    let edam_text = fs::read_to_string(edam_path.as_path())
        .map_err(|source| CliError::ReadEdam { path: edam_path.clone(), source })?;
    let edam: Edam = serde_yaml_ng::from_str(&edam_text)
        .map_err(|source| CliError::ParseEdam { path: edam_path.clone(), source })?;

    project_from_edam(&edam_path, edam)
}

fn project_from_edam(edam_path: &AbsPathBuf, edam: Edam) -> Result<ResolvedProject, CliError> {
    let work_root = edam_path.parent().ok_or(CliError::MissingField { field: "EDAM parent" })?;

    let mut files = Vec::new();
    let mut include_dirs = Vec::new();
    let mut defines = Vec::new();
    for file in edam.files {
        if !is_verilog_source(&file.file_type) {
            continue;
        }

        let path = resolve_path(work_root, &file.name, "files[].name")?;
        let include_path = file
            .include_path
            .as_deref()
            .map(|path| resolve_path(work_root, path, "files[].include_path"))
            .transpose()?;
        let include_path = include_path.or_else(|| {
            file.is_include_file
                .then(|| path.as_path().parent().map(|parent| parent.to_path_buf()))
                .flatten()
        });
        if let Some(include_path) = &include_path {
            include_dirs.push(include_path.clone());
        }

        let file_defines = file
            .define
            .into_iter()
            .map(|(name, value)| value_to_define("files[].define", name, value))
            .collect::<Result<Vec<_>, _>>()?;
        defines.extend(file_defines.iter().cloned());
        files.push(ResolvedFile {
            path,
            file_type: file.file_type,
            is_include_file: file.is_include_file,
            include_path,
            defines: file_defines,
            logical_name: file.logical_name,
        });
    }

    for (name, parameter) in edam.parameters {
        if parameter.paramtype != "vlogdefine" {
            continue;
        }
        let Some(value) = parameter.default else {
            continue;
        };
        defines.push(value_to_define("parameters", name, value)?);
    }

    include_dirs.sort();
    include_dirs.dedup();
    let top_modules = edam
        .toplevel
        .ok_or(CliError::MissingField { field: "toplevel" })?
        .split_whitespace()
        .map(str::to_owned)
        .collect();

    let cores = edam
        .cores
        .into_iter()
        .map(|(name, core)| {
            let core_file = resolve_path(work_root, &core.core_file, "cores[].core_file")?;
            let core_root = core_file.parent().ok_or(CliError::InvalidField {
                field: "cores[].core_file",
                detail: format!("core file has no parent: {core_file}"),
            })?;
            Ok(ResolvedCore { name, core_root: core_root.to_path_buf(), core_file })
        })
        .collect::<Result<Vec<_>, CliError>>()?;

    Ok(ResolvedProject { files, include_dirs, defines, top_modules, cores })
}

fn resolve_path(base: &AbsPath, path: &str, field: &'static str) -> Result<AbsPathBuf, CliError> {
    let path = Utf8PathBuf::from(path);
    if path.is_absolute() {
        AbsPathBuf::try_from(path).map_err(|path| CliError::InvalidField {
            field,
            detail: format!("path is not a valid absolute path: {path}"),
        })
    } else {
        Ok(base.join(path).normalize())
    }
}

fn value_to_define(
    field: &'static str,
    name: String,
    value: Value,
) -> Result<(String, String), CliError> {
    let value = match value {
        Value::String(value) => value,
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        other => {
            return Err(CliError::InvalidField {
                field,
                detail: format!("define `{name}` is not a scalar: {other:?}"),
            });
        }
    };
    Ok((name, value))
}

fn is_verilog_source(file_type: &str) -> bool {
    file_type.to_ascii_lowercase().contains("verilog")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_edam_to_project() {
        let root = tempfile::tempdir().unwrap();
        let root = utils::paths::abs_path_buf_from_path_buf(root.path().to_path_buf()).unwrap();
        let edam_path = root.join("project.eda.yml");
        fs::write(
            edam_path.as_path(),
            r#"
toplevel: top
parameters:
  WIDTH:
    paramtype: vlogdefine
    default: 32
files:
  - name: rtl/top.sv
    file_type: systemVerilogSource
  - name: include/config.vh
    file_type: verilogSource
    is_include_file: true
  - name: constraints.xdc
    file_type: XDC
cores:
  v:l:top:1.0:
    core_file: top.core
"#,
        )
        .unwrap();

        let edam: Edam =
            serde_yaml_ng::from_str(&fs::read_to_string(edam_path.as_path()).unwrap()).unwrap();
        let project = project_from_edam(&edam_path, edam).unwrap();
        assert_eq!(project.top_modules, ["top"]);
        assert_eq!(project.files.len(), 2);
        assert_eq!(project.include_dirs, [root.join("include")]);
        assert!(project.defines.contains(&("WIDTH".to_owned(), "32".to_owned())));
        assert_eq!(project.cores[0].core_file, root.join("top.core"));
    }

    #[test]
    fn reads_core_target_metadata_without_resolving_dependencies() {
        let root = tempfile::tempdir().unwrap();
        let root = utils::paths::abs_path_buf_from_path_buf(root.path().to_path_buf()).unwrap();
        let core_path = root.join("top.core");
        fs::write(
            core_path.as_path(),
            "CAPI=2:\nname: v:l:top:1.0\ntargets:\n  default:\n    filesets: [rtl]\n  lint:\n    description: Run static checks\n    default_tool: verilator\n    toplevel: top\n",
        )
        .unwrap();

        let targets = read_core_targets(&core_path).unwrap();

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].name, "default");
        assert!(!targets[0].has_toplevel);
        assert_eq!(targets[0].source_line, 3);
        assert_eq!(targets[1].name, "lint");
        assert_eq!(targets[1].default_tool.as_deref(), Some("verilator"));
        assert!(targets[1].has_toplevel);
        assert_eq!(targets[1].source_line, 5);
    }
}
