use std::path::Path;

use anyhow::Context;
use base_db::source_root::SourceRootRole;
use ide::{Cancellable, analysis::Analysis};
use lsp_types::Url;
use nohash_hasher::IntMap;
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use project_model::Workspace;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::{
    lines::{LineEnding, LineInfo},
    paths::AbsPathBuf,
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

    pub(crate) fn file_path(&self, file_id: FileId) -> Option<AbsPathBuf> {
        self.vfs_read().file_path(file_id)?.as_abs_path().map(ToOwned::to_owned)
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
        diagnostics.extend(self.qihe_diagnostics(file_id));
        diagnostics
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

    fn source_root_role(&self, file_id: FileId) -> Option<SourceRootRole> {
        self.analysis.source_root_role(file_id).ok()
    }

    pub(crate) fn file_allows_workspace_edits(&self, file_id: FileId) -> bool {
        self.source_root_role(file_id).is_none_or(SourceRootRole::allows_workspace_edits)
    }

    pub(crate) fn workspace_diagnostic_file_ids(&self) -> Vec<FileId> {
        let open_files = self.mem_docs.file_ids().collect::<FxHashSet<_>>();
        self.file_ids()
            .into_iter()
            .filter(|file_id| {
                open_files.contains(file_id)
                    || self
                        .source_root_role(*file_id)
                        .is_none_or(SourceRootRole::publishes_unopened_workspace_diagnostics)
            })
            .collect()
    }

    pub(crate) fn file_ids(&self) -> Vec<FileId> {
        let vfs = self.vfs.read();
        vfs.0.iter().map(|(file_id, _)| file_id).collect()
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
