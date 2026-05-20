use std::fs;

use project_model::{toml_manifest_diagnostics, toml_manifest_paths};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

use crate::{global_state::snapshot::GlobalStateSnapshot, lsp_ext::to_proto};

const MANIFEST_DIAGNOSTIC_CODE: &str = "manifest";
const MANIFEST_PATH_DIAGNOSTIC_CODE: &str = "manifest.path";

pub(crate) fn diagnostics(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
) -> Vec<lsp_types::Diagnostic> {
    if !snap.is_manifest_file(file_id) {
        return Vec::new();
    }

    let Ok(text) = snap.file_text(file_id) else {
        return Vec::new();
    };
    let Ok(line_info) = snap.line_info(file_id) else {
        return Vec::new();
    };

    let schema_diagnostics = toml_manifest_diagnostics(&text);
    if !schema_diagnostics.is_empty() {
        return schema_diagnostics
            .into_iter()
            .map(|diag| {
                let range = diag
                    .range
                    .map(|range| byte_range_to_text_range(range, text.len()))
                    .unwrap_or_else(|| TextRange::empty(TextSize::new(0)));
                lsp_types::Diagnostic {
                    range: to_proto::range(&line_info, range),
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    code: Some(lsp_types::NumberOrString::String(
                        MANIFEST_DIAGNOSTIC_CODE.to_string(),
                    )),
                    code_description: None,
                    source: Some("vizsla".to_string()),
                    message: diag.message,
                    related_information: None,
                    tags: None,
                    data: None,
                }
            })
            .collect();
    }

    missing_path_diagnostics(snap, file_id, &text, &line_info)
}

fn missing_path_diagnostics(
    snap: &GlobalStateSnapshot,
    file_id: FileId,
    text: &str,
    line_info: &utils::lines::LineInfo,
) -> Vec<lsp_types::Diagnostic> {
    let Some(manifest_path) = snap.file_abs_path(file_id) else {
        return Vec::new();
    };
    let Some(manifest_dir) = manifest_path.parent() else {
        return Vec::new();
    };

    toml_manifest_paths(text)
        .into_iter()
        .filter(|path| path.key != "exclude")
        .filter_map(|path| {
            let target = manifest_dir.absolutize(path.value.replace('\\', "/"));
            if fs::metadata(target.as_path()).is_ok() {
                return None;
            }

            let range = byte_range_to_text_range(path.content_range.clone(), text.len());
            Some(lsp_types::Diagnostic {
                range: to_proto::range(line_info, range),
                severity: Some(lsp_types::DiagnosticSeverity::WARNING),
                code: Some(lsp_types::NumberOrString::String(
                    MANIFEST_PATH_DIAGNOSTIC_CODE.to_string(),
                )),
                code_description: None,
                source: Some("vizsla".to_string()),
                message: format!("manifest path does not exist for `{}`: {}", path.key, path.value),
                related_information: None,
                tags: None,
                data: None,
            })
        })
        .collect()
}

fn byte_range_to_text_range(range: std::ops::Range<usize>, text_len: usize) -> TextRange {
    fn to_text_size(value: usize) -> TextSize {
        TextSize::new(u32::try_from(value).unwrap_or(u32::MAX))
    }

    let start = range.start.min(text_len);
    let end = range.end.min(text_len).max(start);
    TextRange::new(to_text_size(start), to_text_size(end))
}
