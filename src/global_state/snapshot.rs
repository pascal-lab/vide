use std::path::Path;

use anyhow::Context;
use ide::{Cancellable, analysis::Analysis};
use lsp_types::Url;
use nohash_hasher::IntMap;
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use project_model::{
    Workspace, project_manifest::is_manifest_file_name, toml_manifest_diagnostics,
};
use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::{
    lines::{LineEnding, LineInfo},
    paths::AbsPathBuf,
    text_edit::{TextRange, TextSize},
};
use vfs::{FileId, Vfs, VfsPath};

use super::mem_docs::MemDocs;
use crate::{
    config::Config,
    global_state::QiheDiagnosticState,
    lsp_ext::{from_proto, to_proto},
};

// immutable
pub(crate) struct GlobalStateSnapshot {
    pub(crate) config: Arc<Config>,
    pub(crate) analysis: Analysis,
    // pub(crate) check_fixes: CheckFixes,
    pub(crate) sema_tokens_cache: Arc<Mutex<FxHashMap<Url, lsp_types::SemanticTokens>>>,
    pub(crate) qihe_diagnostics: Arc<Mutex<FxHashMap<FileId, QiheDiagnosticState>>>,
    pub(crate) mem_docs: MemDocs,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEnding>)>>,
    #[allow(dead_code)]
    pub(crate) workspaces: Arc<Vec<Workspace>>,
}

impl std::panic::UnwindSafe for GlobalStateSnapshot {}

impl GlobalStateSnapshot {
    fn vfs_read(&self) -> MappedRwLockReadGuard<'_, Vfs> {
        RwLockReadGuard::map(self.vfs.read(), |(it, _)| it)
    }

    pub(crate) fn file_id(&self, url: &lsp_types::Url) -> anyhow::Result<FileId> {
        let path = from_proto::vfs_path(url)?;
        let vfs = self.vfs_read();
        let file_id =
            vfs.file_id(&path).ok_or_else(|| anyhow::format_err!("file not found: {path}"))?;
        Ok(file_id)
    }

    pub(crate) fn file_id_for_path(&self, path: &Path) -> Option<FileId> {
        let path = VfsPath::from(AbsPathBuf::try_from(path.to_path_buf()).ok()?);
        self.vfs_read().file_id(&path)
    }

    pub(crate) fn line_info(&self, file_id: FileId) -> anyhow::Result<LineInfo> {
        let ending = {
            let vfs = self.vfs.read();
            vfs.1
                .get(&file_id)
                .copied()
                .or_else(|| vfs.0.line_ending(file_id))
                .with_context(|| format!("missing line ending metadata for {file_id:?}"))?
        };
        let index = self.analysis.line_index(file_id)?;
        let encoding = self.config.position_encoding();
        let res = LineInfo { index, ending, encoding };
        Ok(res)
    }

    pub(crate) fn file_text(&self, file_id: FileId) -> Cancellable<Arc<str>> {
        let text = self.analysis.file_text(file_id)?;
        Ok(text)
    }

    pub(crate) fn diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<ide::diagnostics::Diagnostic>> {
        let diagnostics = if self.config.diagnostics_config().semantic.enabled {
            self.analysis.diagnostics(file_id)?
        } else {
            self.analysis.parse_diagnostics(file_id)?
        };

        Ok(diagnostics)
    }

    pub(crate) fn source_root_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<ide::diagnostics::Diagnostic>> {
        let diagnostics = if self.config.diagnostics_config().semantic.enabled {
            self.analysis.source_root_diagnostics(file_id)?
        } else {
            self.analysis.parse_diagnostics(file_id)?
        };

        Ok(diagnostics)
    }

    pub(crate) fn lsp_diagnostics(&self, file_id: FileId) -> Vec<lsp_types::Diagnostic> {
        let mut diagnostics = match (self.diagnostics(file_id), self.line_info(file_id)) {
            (Ok(diagnostics), Ok(line_info)) => diagnostics
                .into_iter()
                .map(|diag| crate::lsp_ext::to_proto::diagnostic(&line_info, diag))
                .collect(),
            _ => Vec::new(),
        };
        diagnostics.extend(self.manifest_lsp_diagnostics(file_id));
        diagnostics.extend(self.qihe_diagnostics(file_id));
        diagnostics
    }

    pub(crate) fn manifest_lsp_diagnostics(&self, file_id: FileId) -> Vec<lsp_types::Diagnostic> {
        if !self.is_manifest_file(file_id) {
            return Vec::new();
        }

        let Ok(text) = self.file_text(file_id) else {
            return Vec::new();
        };
        let Ok(line_info) = self.line_info(file_id) else {
            return Vec::new();
        };

        toml_manifest_diagnostics(&text)
            .into_iter()
            .map(|diag| {
                let range = diag
                    .range
                    .map(|range| byte_range_to_text_range(range, text.len()))
                    .unwrap_or_else(|| TextRange::empty(TextSize::new(0)));
                lsp_types::Diagnostic {
                    range: to_proto::range(&line_info, range),
                    severity: Some(lsp_types::DiagnosticSeverity::ERROR),
                    code: Some(lsp_types::NumberOrString::String("manifest".to_string())),
                    code_description: None,
                    source: Some("vizsla".to_string()),
                    message: diag.message,
                    related_information: None,
                    tags: None,
                    data: None,
                }
            })
            .collect()
    }

    pub(crate) fn qihe_diagnostics(&self, file_id: FileId) -> Vec<lsp_types::Diagnostic> {
        self.qihe_diagnostics
            .lock()
            .get(&file_id)
            .map(|state| state.diagnostics.clone())
            .unwrap_or_default()
    }

    pub(crate) fn qihe_generation(&self, file_id: FileId) -> u64 {
        self.qihe_diagnostics.lock().get(&file_id).map(|state| state.generation).unwrap_or(0)
    }

    pub(crate) fn file_version(&self, file_id: FileId) -> Option<i32> {
        self.mem_docs.version(file_id)
    }

    pub(crate) fn diagnostic_result_id(&self, file_id: FileId) -> Option<String> {
        let diagnostics_config = self.config.diagnostics_config();
        let file_ids = if diagnostics_config.semantic.enabled {
            self.analysis.source_root_file_ids(file_id).ok()?
        } else {
            vec![file_id]
        };

        let mut versions = file_ids
            .into_iter()
            .filter_map(|file_id| self.file_version(file_id).map(|version| (file_id.0, version)))
            .collect::<Vec<_>>();

        if versions.is_empty() {
            return None;
        }

        versions.sort_unstable();
        let file_versions = versions
            .into_iter()
            .map(|(file_id, version)| format!("{file_id}:{version}"))
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "diag:{}:{file_versions}:qihe:{}",
            diagnostics_config.revision,
            self.qihe_generation(file_id)
        ))
    }

    pub(crate) fn source_root_file_ids(&self, file_id: FileId) -> Vec<FileId> {
        self.analysis.source_root_file_ids(file_id).unwrap_or_else(|_| vec![file_id])
    }

    pub(crate) fn file_ids(&self) -> Vec<FileId> {
        let vfs = self.vfs.read();
        vfs.0.iter().map(|(file_id, _)| file_id).collect()
    }

    pub(crate) fn is_manifest_file(&self, file_id: FileId) -> bool {
        let vfs = self.vfs_read();
        vfs.file_path(file_id)
            .and_then(|path| path.as_abs_path())
            .and_then(|path| path.file_name())
            .is_some_and(is_manifest_file_name)
    }

    pub(crate) fn url(&self, id: FileId) -> anyhow::Result<Url> {
        let vfs = &self.vfs_read();
        let path =
            vfs.file_path(id).ok_or_else(|| anyhow::format_err!("unknown file id: {id:?}"))?;
        let path = path
            .as_abs_path()
            .ok_or_else(|| anyhow::format_err!("file {id:?} has no file URI: {path}"))?;
        to_proto::url_from_abs_path(path)
    }

    pub(crate) fn url_file_version(&self, url: &Url) -> Option<i32> {
        let path = from_proto::vfs_path(url).ok()?;
        self.mem_docs.file_id(&path).and_then(|file_id| self.file_version(file_id))
    }
}

fn byte_range_to_text_range(range: std::ops::Range<usize>, text_len: usize) -> TextRange {
    fn to_text_size(value: usize) -> TextSize {
        TextSize::new(u32::try_from(value).unwrap_or(u32::MAX))
    }

    let start = range.start.min(text_len);
    let end = range.end.min(text_len).max(start);
    TextRange::new(to_text_size(start), to_text_size(end))
}
