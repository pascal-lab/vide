use std::{path::Path, sync::Arc as StdArc};

use hir::base_db::source_root::{SourceRootDiagnosticScope, SourceRootRole};
use ide::{Cancellable, analysis::AnalysisSnapshot};
use lsp_types::Url;
use nohash_hasher::IntMap;
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use project_model::Workspace;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::{
    cancellation::CancellationToken,
    lines::{LineEnding, LineInfo},
    paths::AbsPathBuf,
};
use vfs::{FileId, Vfs, VfsPath};

use super::{
    diagnostics::{
        DiagnosticCommitFreshness, DiagnosticFileRevision, DiagnosticOwner,
        DiagnosticPublishFreshness, DiagnosticRequestScope, DiagnosticSnapshotKey,
        DiagnosticSource, DiagnosticWorkspaceProducer,
    },
    mem_docs::MemDocs,
    response_effect::{AcceptedResponseEffect, AcceptedResponseEffects},
};
use crate::{
    config::Config,
    lsp_ext::{from_proto, to_proto},
};

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticPublishTarget {
    /// The analysis identity diagnostics are computed from.
    file_id: FileId,
    /// The URI diagnostics will be published for.
    ///
    /// Diagnostic code should obtain URI/version pairs from
    /// [`GlobalStateSnapshot::diagnostic_publish_targets`] instead of pairing
    /// [`GlobalStateSnapshot::url`] with a file-id-wide document version.
    uri: Url,
    /// The document version for `uri`, when that URI is currently open.
    version: Option<i32>,
}

impl DiagnosticPublishTarget {
    fn new(file_id: FileId, uri: Url, version: Option<i32>) -> Self {
        Self { file_id, uri, version }
    }

    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }

    pub(crate) fn version(&self) -> Option<i32> {
        self.version
    }

    pub(crate) fn into_parts(self) -> (FileId, Url, Option<i32>) {
        (self.file_id, self.uri, self.version)
    }
}

// immutable
pub(crate) struct GlobalStateSnapshot {
    pub(crate) config: Arc<Config>,
    pub(crate) analysis: AnalysisSnapshot,
    // pub(crate) check_fixes: CheckFixes,
    pub(crate) sema_tokens_cache: Arc<Mutex<FxHashMap<Url, lsp_types::SemanticTokens>>>,
    pub(crate) external_sources: Vec<StdArc<dyn DiagnosticSource>>,
    pub(crate) diagnostic_publish_freshness: DiagnosticPublishFreshness,
    pub(crate) diagnostic_file_revisions: FxHashMap<FileId, DiagnosticFileRevision>,
    pub(crate) cancellation: CancellationToken,
    pub(super) accepted_response_effects: AcceptedResponseEffects,
    pub(crate) mem_docs: MemDocs,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEnding>)>>,
    #[allow(dead_code)]
    pub(crate) workspaces: Arc<Vec<Workspace>>,
}

impl std::fmt::Debug for GlobalStateSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GlobalStateSnapshot")
            .field("snapshot_id", &self.analysis_snapshot_id())
            .field("diagnostic_publish_freshness", &self.diagnostic_publish_freshness)
            .finish()
    }
}

impl std::panic::UnwindSafe for GlobalStateSnapshot {}

impl GlobalStateSnapshot {
    pub(crate) fn analysis_snapshot_id(
        &self,
    ) -> hir::base_db::analysis_snapshot::AnalysisSnapshotId {
        self.analysis.snapshot_id()
    }

    fn vfs_read(&self) -> MappedRwLockReadGuard<'_, Vfs> {
        RwLockReadGuard::map(self.vfs.read(), |(it, _)| it)
    }

    pub(crate) fn file_id(&self, url: &lsp_types::Url) -> anyhow::Result<FileId> {
        let path = from_proto::vfs_path(url)?;
        let vfs = self.vfs_read();
        let (file_id, _) =
            vfs.file_id(&path).ok_or_else(|| anyhow::format_err!("file not found: {path}"))?;
        Ok(file_id)
    }

    pub(crate) fn on_response_accepted(&self, effect: AcceptedResponseEffect) {
        self.accepted_response_effects.push(effect);
    }

    pub(crate) fn accepted_response_effects(&self) -> AcceptedResponseEffects {
        self.accepted_response_effects.clone()
    }

    pub(crate) fn file_id_for_path(&self, path: &Path) -> Option<FileId> {
        let path = VfsPath::from(AbsPathBuf::try_from(path.to_path_buf()).ok()?);
        self.vfs_read().file_id(&path).map(|(file_id, _)| file_id)
    }

    pub(crate) fn file_path(&self, file_id: FileId) -> Option<AbsPathBuf> {
        self.vfs_read().file_path(file_id).as_abs_path().map(ToOwned::to_owned)
    }

    pub(crate) fn line_info(&self, file_id: FileId) -> anyhow::Result<LineInfo> {
        let ending = {
            let vfs = self.vfs.read();
            vfs.1.get(&file_id).copied().unwrap_or(utils::lines::LineEnding::Unix)
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
        if !self.document_diagnostics_enabled(file_id) {
            return Ok(Vec::new());
        }

        if self.open_file_syntax_diagnostics_for_disabled_root(file_id) {
            return self.analysis.parse_diagnostics(file_id);
        }

        if let Some(DiagnosticOwner::CompilationProfile(profile_id)) =
            self.diagnostic_owner(file_id, DiagnosticRequestScope::Document)
        {
            let diagnostics = self.analysis.compilation_profile_diagnostics(profile_id)?;
            return Ok(diagnostics.into_iter().filter(|diag| diag.file_id == file_id).collect());
        }

        self.analysis.diagnostics(file_id)
    }

    pub(crate) fn lsp_diagnostics(
        &self,
        file_id: FileId,
    ) -> anyhow::Result<Vec<lsp_types::Diagnostic>> {
        tracing::debug!(
            snapshot_id = ?self.analysis_snapshot_id(),
            ?file_id,
            "resolve diagnostics for analysis snapshot"
        );
        if !self.document_diagnostics_enabled(file_id) {
            return Ok(Vec::new());
        }

        let diagnostics = self.diagnostics(file_id)?;
        let line_info = self.line_info(file_id)?;
        let mut diagnostics = diagnostics
            .into_iter()
            .map(|diag| crate::lsp_ext::to_proto::diagnostic(self.config.i18n, &line_info, diag))
            .collect::<Vec<_>>();
        diagnostics.extend(self.external_lsp_diagnostics(file_id)?);
        Ok(diagnostics)
    }

    pub(crate) fn external_diagnostics(
        &self,
        file_id: FileId,
    ) -> Vec<ide::diagnostics::Diagnostic> {
        let freshness = self.diagnostic_commit_freshness();
        self.external_sources
            .iter()
            .flat_map(|source| source.diagnostics(file_id, &freshness))
            .collect()
    }

    pub(crate) fn external_lsp_diagnostics(
        &self,
        file_id: FileId,
    ) -> anyhow::Result<Vec<lsp_types::Diagnostic>> {
        let freshness = self.diagnostic_commit_freshness();
        let mut diagnostics = Vec::new();
        for source in &self.external_sources {
            for diagnostic in source.diagnostics(file_id, &freshness) {
                let line_info = self.line_info(diagnostic.file_id)?;
                diagnostics.push(to_proto::diagnostic(self.config.i18n, &line_info, diagnostic));
            }
            diagnostics.extend(source.lsp_diagnostics(file_id, &freshness));
        }
        Ok(diagnostics)
    }

    pub(crate) fn diagnostic_commit_freshness(&self) -> DiagnosticCommitFreshness {
        self.diagnostic_publish_freshness.commit()
    }

    /// Returns a result id scoped to the URI that receives the diagnostics.
    ///
    /// The same physical file may be open through an alias URI, so result ids
    /// must not be derived from [`FileId`] alone.
    pub(crate) fn document_diagnostic_result_id(
        &self,
        file_id: FileId,
        target_uri: &Url,
    ) -> Option<String> {
        self.diagnostic_result_id(file_id, target_uri, DiagnosticRequestScope::Document)
    }

    pub(crate) fn workspace_diagnostic_result_id(
        &self,
        file_id: FileId,
        target_uri: &Url,
    ) -> Option<String> {
        self.diagnostic_result_id(file_id, target_uri, DiagnosticRequestScope::Workspace)
    }

    fn diagnostic_result_id(
        &self,
        file_id: FileId,
        target_uri: &Url,
        scope: DiagnosticRequestScope,
    ) -> Option<String> {
        let owner = self.diagnostic_owner(file_id, scope)?;
        let file_ids = self.diagnostic_owner_file_ids(owner, file_id)?;
        let target_version = self.url_file_version(target_uri);

        let revisions = file_ids
            .into_iter()
            .map(|file_id| {
                (file_id, self.diagnostic_file_revisions.get(&file_id).copied().unwrap_or_default())
            })
            .collect::<Vec<_>>();
        let freshness = self.diagnostic_commit_freshness();
        let external_revisions = self
            .external_sources
            .iter()
            .filter_map(|source| source.external_revision(file_id, &freshness))
            .collect();
        Some(
            DiagnosticSnapshotKey::new_with_snapshot_id(
                freshness.snapshot_id(),
                owner,
                self.diagnostic_publish_freshness.commit().readiness_revision(),
                self.config.diagnostics_config().revision,
                target_uri,
                target_version,
                revisions,
                external_revisions,
            )
            .result_id(),
        )
    }

    pub(crate) fn diagnostic_owner(
        &self,
        file_id: FileId,
        scope: DiagnosticRequestScope,
    ) -> Option<DiagnosticOwner> {
        let source_root_id = self.analysis.source_root_id(file_id).ok()?;
        let source_root_role = self.source_root_role(file_id)?;
        match source_root_role.diagnostic_scope() {
            SourceRootDiagnosticScope::Disabled => {
                // Explicitly profiled workspaces treat ignored roots as outside
                // the diagnostic model. A workspace with no compilation
                // profiles still allows open-file syntax diagnostics.
                if matches!(scope, DiagnosticRequestScope::Document)
                    && (!self.analysis.has_compilation_profiles().ok()?
                        || self.open_file_syntax_diagnostics_for_disabled_root(file_id))
                {
                    return Some(DiagnosticOwner::File(file_id));
                }
                return None;
            }
            SourceRootDiagnosticScope::OpenFile => return Some(DiagnosticOwner::File(file_id)),
            SourceRootDiagnosticScope::Workspace => {}
        }

        // Compilation profiles own both semantic and syntax-only diagnostics:
        // profile preprocessing/include buffers can change syntax trees even
        // when semantic diagnostics are disabled.
        if let Some(profile_id) = self.analysis.file_compilation_profile(file_id).ok()? {
            return Some(DiagnosticOwner::CompilationProfile(profile_id));
        }

        // Workspace-capable roots without an explicit manifest profile still
        // need one source-root-scoped producer for workspace diagnostics.
        if matches!(scope, DiagnosticRequestScope::Workspace) {
            Some(DiagnosticOwner::SourceRoot(source_root_id))
        } else {
            Some(DiagnosticOwner::File(file_id))
        }
    }

    fn document_diagnostics_enabled(&self, file_id: FileId) -> bool {
        self.diagnostic_owner(file_id, DiagnosticRequestScope::Document).is_some()
    }

    fn open_file_syntax_diagnostics_for_disabled_root(&self, file_id: FileId) -> bool {
        self.mem_docs.contains_file_id(file_id)
            && self.file_is_in_syntax_only_workspace(file_id)
            && !self.file_is_manifest_excluded(file_id)
    }

    fn file_is_in_syntax_only_workspace(&self, file_id: FileId) -> bool {
        let Some(path) = self.file_path(file_id) else {
            return false;
        };

        self.workspaces.iter().any(|workspace| {
            if workspace.is_lib() || !path.starts_with(workspace.root()) {
                return false;
            }

            let roots = workspace.roots();
            !roots.is_empty()
                && roots.iter().any(|root| matches!(root.role, SourceRootRole::Local))
                && roots.iter().all(|root| {
                    root.source.is_empty()
                        && root.source_directories.is_empty()
                        && root.source_files.is_empty()
                        && root.include_dirs.is_empty()
                })
        })
    }

    fn file_is_manifest_excluded(&self, file_id: FileId) -> bool {
        let Some(path) = self.file_path(file_id) else {
            return false;
        };

        self.workspaces.iter().any(|workspace| {
            path.starts_with(workspace.root())
                && workspace.roots().iter().any(|root| {
                    root.exclude_globs
                        .as_ref()
                        .is_some_and(|exclude| exclude.is_match(path.as_path()))
                })
        })
    }

    fn diagnostic_owner_file_ids(
        &self,
        owner: DiagnosticOwner,
        representative_file_id: FileId,
    ) -> Option<Vec<FileId>> {
        let file_ids = match owner {
            DiagnosticOwner::CompilationProfile(profile_id) => {
                self.analysis.compilation_profile_file_ids(profile_id).ok()?
            }
            DiagnosticOwner::SourceRoot(_) => {
                self.analysis.source_root_file_ids(representative_file_id).ok()?
            }
            DiagnosticOwner::File(file_id) | DiagnosticOwner::External { file: file_id, .. } => {
                vec![file_id]
            }
        };
        Some(file_ids)
    }

    pub(crate) fn workspace_diagnostic_producers(
        &self,
        file_ids: &[FileId],
    ) -> Vec<DiagnosticWorkspaceProducer> {
        let mut owners = FxHashMap::default();
        for file_id in file_ids {
            let Some(owner) = self.diagnostic_owner(*file_id, DiagnosticRequestScope::Workspace)
            else {
                continue;
            };
            owners.entry(owner).or_insert(*file_id);
        }

        owners
            .into_iter()
            .map(|(owner, representative_file_id)| {
                DiagnosticWorkspaceProducer::new(owner, representative_file_id)
            })
            .collect()
    }

    pub(crate) fn workspace_diagnostics_for_producer(
        &self,
        producer: &DiagnosticWorkspaceProducer,
    ) -> Cancellable<Vec<ide::diagnostics::Diagnostic>> {
        match producer.owner() {
            DiagnosticOwner::CompilationProfile(profile_id) => {
                self.analysis.compilation_profile_diagnostics(profile_id)
            }
            DiagnosticOwner::SourceRoot(_) => {
                self.analysis.source_root_diagnostics(producer.representative_file_id())
            }
            DiagnosticOwner::File(file_id) => self.diagnostics(file_id),
            DiagnosticOwner::External { .. } => Ok(Vec::new()),
        }
    }

    pub(crate) fn compilation_profile_ids(
        &self,
    ) -> Cancellable<Vec<hir::base_db::project::CompilationProfileId>> {
        self.analysis.compilation_profile_ids()
    }

    pub(crate) fn diagnostic_target_file_ids_for_changes(
        &self,
        changed_file_ids: &FxHashSet<FileId>,
        candidate_file_ids: impl IntoIterator<Item = FileId>,
    ) -> FxHashSet<FileId> {
        let changed_owners = changed_file_ids
            .iter()
            .filter_map(|file_id| self.diagnostic_owner(*file_id, DiagnosticRequestScope::Document))
            .collect::<FxHashSet<_>>();
        if changed_owners.is_empty() {
            return FxHashSet::default();
        }

        candidate_file_ids
            .into_iter()
            .filter(|file_id| {
                self.diagnostic_owner(*file_id, DiagnosticRequestScope::Document)
                    .is_some_and(|owner| changed_owners.contains(&owner))
            })
            .collect()
    }

    fn source_root_role(&self, file_id: FileId) -> Option<SourceRootRole> {
        self.analysis.source_root_role(file_id).ok()
    }

    pub(crate) fn rename_config(&self, file_id: FileId) -> ide::rename::RenameConfig {
        let mut config = self.config.rename();
        if matches!(self.source_root_role(file_id), Some(SourceRootRole::BestEffortIndex)) {
            config = config.with_edit_scope(ide::rename::RenameEditScope::SingleFile);
        }
        config
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
        self.analysis.file_ids().unwrap_or_default()
    }

    /// Returns the VFS primary URI for a file.
    ///
    /// This is suitable for protocol features that need a stable file location.
    /// Push diagnostics must use [`Self::diagnostic_publish_targets`] so the
    /// URI and document version come from the same open document spelling.
    pub(crate) fn url(&self, id: FileId) -> anyhow::Result<Url> {
        let vfs = &self.vfs_read();
        let path = vfs.file_path(id);
        let path = path
            .as_abs_path()
            .ok_or_else(|| anyhow::format_err!("file {id:?} has no file URI: {path}"))?;
        to_proto::url_from_abs_path(path)
    }

    /// Returns the open URI/version pairs diagnostics should be published to.
    ///
    /// Push diagnostics are scoped to open documents. Workspace diagnostic
    /// requests use [`GlobalStateSnapshot::workspace_diagnostic_targets`] when
    /// they need to report unopened workspace files.
    pub(crate) fn diagnostic_publish_targets(
        &self,
        file_id: FileId,
    ) -> anyhow::Result<Vec<DiagnosticPublishTarget>> {
        self.open_diagnostic_targets(file_id)
    }

    pub(crate) fn workspace_diagnostic_targets(
        &self,
        file_id: FileId,
    ) -> anyhow::Result<Vec<DiagnosticPublishTarget>> {
        let open_targets = self.open_diagnostic_targets(file_id)?;
        if !open_targets.is_empty() {
            return Ok(open_targets);
        }

        Ok(vec![DiagnosticPublishTarget::new(file_id, self.url(file_id)?, None)])
    }

    fn open_diagnostic_targets(
        &self,
        file_id: FileId,
    ) -> anyhow::Result<Vec<DiagnosticPublishTarget>> {
        let open_documents = self.mem_docs.open_documents(file_id);
        open_documents
            .into_iter()
            .map(|document| {
                let path = document.path.as_abs_path().ok_or_else(|| {
                    anyhow::format_err!("open file {file_id:?} has no file URI: {}", document.path)
                })?;
                let uri = to_proto::url_from_abs_path(path)?;
                Ok(DiagnosticPublishTarget::new(file_id, uri, Some(document.version)))
            })
            .collect()
    }

    pub(crate) fn url_file_version(&self, url: &Url) -> Option<i32> {
        let path = from_proto::vfs_path(url).ok()?;
        self.mem_docs.version_for_path(&path)
    }
}

#[cfg(test)]
mod tests {
    use lsp_server::Connection;
    use lsp_types::{ClientCapabilities, TraceValue};
    use utils::test_support::TestDir;
    use vfs::{FileId, VfsPath /* removed */};

    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    #[test]
    fn snapshot_file_ids_stay_in_analysis_snapshot() {
        let root = TestDir::new("snapshot-file-ids-analysis-boundary");
        let root_path = root.path().to_path_buf();
        let config = config::Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, _client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, TraceValue::Off);

        state.workspace.vfs.write().0.set_file_contents(
            VfsPath::from(root.join("top.sv")),
            Some(b"module top; endmodule\n".to_vec()),
        );
        assert!(state.process_changes());
        let snapshot = state.make_snapshot();

        state.workspace.vfs.write().0.set_file_contents(
            VfsPath::from(root.join("child.sv")),
            Some(b"module child; endmodule\n".to_vec()),
        );

        let mut live_file_ids =
            state.workspace.vfs.read().0.iter().map(|(file_id, _)| file_id).collect::<Vec<_>>();
        live_file_ids.sort_unstable_by_key(|file_id| file_id.index());
        assert_eq!(live_file_ids, vec![FileId::from_raw(0), FileId::from_raw(1)]);

        assert_eq!(snapshot.file_ids(), vec![FileId::from_raw(0)]);
    }
}
