use ide::{Cancellable, analysis::Analysis};
use lsp_types::Url;
use nohash_hasher::IntMap;
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use project_model::Workspace;
use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::lines::{LineEnding, LineInfo};
use vfs::{FileId, Vfs};

use super::mem_docs::MemDocs;
use crate::{
    config::{Config, user_config::VerilogModelUnsupportedConstructs},
    lsp_ext::{from_proto, to_proto},
};

// immutable
pub(crate) struct GlobalStateSnapshot {
    pub(crate) config: Arc<Config>,
    pub(crate) analysis: Analysis,
    // pub(crate) check_fixes: CheckFixes,
    pub(crate) sema_tokens_cache: Arc<Mutex<FxHashMap<Url, lsp_types::SemanticTokens>>>,
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

    pub(crate) fn line_info(&self, file_id: FileId) -> Cancellable<LineInfo> {
        let ending = self.vfs.read().1[&file_id];
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
        let mut diagnostics = if self.config.diagnostics_config().semantic.enabled {
            self.analysis.diagnostics(file_id)?
        } else {
            self.analysis.parse_diagnostics(file_id)?
        };

        self.extend_model_limit_diagnostics([file_id], &mut diagnostics)?;
        Ok(diagnostics)
    }

    pub(crate) fn source_root_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<ide::diagnostics::Diagnostic>> {
        let mut diagnostics = if self.config.diagnostics_config().semantic.enabled {
            self.analysis.source_root_diagnostics(file_id)?
        } else {
            self.analysis.parse_diagnostics(file_id)?
        };

        self.extend_model_limit_diagnostics(self.source_root_file_ids(file_id), &mut diagnostics)?;
        Ok(diagnostics)
    }

    fn extend_model_limit_diagnostics(
        &self,
        file_ids: impl IntoIterator<Item = FileId>,
        diagnostics: &mut Vec<ide::diagnostics::Diagnostic>,
    ) -> Cancellable<()> {
        if self.config.user_config.verilog.model_unsupported_constructs
            != VerilogModelUnsupportedConstructs::Diagnostic
        {
            return Ok(());
        }

        for file_id in file_ids {
            diagnostics.extend(self.analysis.model_limit_diagnostics(file_id)?);
        }

        Ok(())
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
            "diag:{}:{:?}:{file_versions}",
            diagnostics_config.revision,
            self.config.user_config.verilog.model_unsupported_constructs
        ))
    }

    pub(crate) fn source_root_file_ids(&self, file_id: FileId) -> Vec<FileId> {
        self.analysis.source_root_file_ids(file_id).unwrap_or_else(|_| vec![file_id])
    }

    pub(crate) fn file_ids(&self) -> Vec<FileId> {
        let vfs = self.vfs.read();
        vfs.0.iter().map(|(file_id, _)| file_id).collect()
    }

    pub(crate) fn url(&self, id: FileId) -> Url {
        let vfs = &self.vfs_read();
        let path = vfs.file_path(id);
        let path = path.as_abs_path().unwrap();
        to_proto::url_from_abs_path(path)
    }

    pub(crate) fn url_file_version(&self, url: &Url) -> Option<i32> {
        let path = from_proto::vfs_path(url).ok()?;
        self.mem_docs.file_id(&path).and_then(|file_id| self.file_version(file_id))
    }
}
