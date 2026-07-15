use hir::base_db::{
    analysis_snapshot::AnalysisSnapshotId, project::CompilationProfileId, source_root::SourceRootId,
};
use lsp_types::Url;
use rustc_hash::FxHashSet;
use vfs::FileId;

pub(crate) mod publisher;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiagnosticCommitFreshness {
    snapshot_id: AnalysisSnapshotId,
    diagnostics_revision: u64,
    readiness_revision: u64,
}

impl DiagnosticCommitFreshness {
    pub(crate) fn for_snapshot(
        snapshot_id: AnalysisSnapshotId,
        diagnostics_revision: u64,
        readiness_revision: u64,
    ) -> Self {
        Self { snapshot_id, diagnostics_revision, readiness_revision }
    }

    pub(crate) fn snapshot_id(self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub(crate) fn readiness_revision(self) -> u64 {
        self.readiness_revision
    }
}

pub(crate) trait DiagnosticSource: Send + Sync {
    /// Domain diagnostics consumed by IDE features before protocol conversion.
    fn diagnostics(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Vec<ide::diagnostics::Diagnostic> {
        let _ = (file_id, freshness);
        Vec::new()
    }

    /// Protocol-native diagnostics for sources that are produced outside the
    /// IDE model.
    fn lsp_diagnostics(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Vec<lsp_types::Diagnostic> {
        let _ = (file_id, freshness);
        Vec::new()
    }

    fn external_revision(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Option<DiagnosticExternalRevision>;

    fn remove_deleted(&self, files: &FxHashSet<FileId>);
}

/// Freshness token for a diagnostics publish batch.
///
/// Diagnostic contents and diagnostic publish targets can change
/// independently. VFS/content/config/readiness changes advance commit
/// freshness; didOpen/didClose and identity remaps advance `target_revision`
/// because they change which URIs are live publish targets without necessarily
/// changing the analysis text. External diagnostics carry this same commit
/// freshness plus their own per-owner generation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiagnosticPublishFreshness {
    commit: DiagnosticCommitFreshness,
    target_revision: u64,
}

impl DiagnosticPublishFreshness {
    pub(crate) fn new(
        snapshot_id: AnalysisSnapshotId,
        diagnostics_revision: u64,
        target_revision: u64,
        readiness_revision: u64,
    ) -> Self {
        Self {
            commit: DiagnosticCommitFreshness::for_snapshot(
                snapshot_id,
                diagnostics_revision,
                readiness_revision,
            ),
            target_revision,
        }
    }

    pub(crate) fn commit(self) -> DiagnosticCommitFreshness {
        self.commit
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(crate) enum DiagnosticOwner {
    File(FileId),
    SourceRoot(SourceRootId),
    CompilationProfile(CompilationProfileId),
    External { source: &'static str, file: FileId },
}

impl DiagnosticOwner {
    fn result_id_fragment(self) -> String {
        match self {
            DiagnosticOwner::File(file_id) => format!("file:{}", file_id.index()),
            DiagnosticOwner::SourceRoot(source_root_id) => {
                format!("source-root:{}", source_root_id.0)
            }
            DiagnosticOwner::CompilationProfile(profile_id) => {
                format!("compilation-profile:{}", profile_id.0)
            }
            DiagnosticOwner::External { source, file } => {
                format!("external-{source}:{}", file.index())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DiagnosticRequestScope {
    Document,
    Workspace,
}

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticWorkspaceProducer {
    owner: DiagnosticOwner,
    representative_file_id: FileId,
}

impl DiagnosticWorkspaceProducer {
    pub(crate) fn new(owner: DiagnosticOwner, representative_file_id: FileId) -> Self {
        Self { owner, representative_file_id }
    }

    pub(crate) fn owner(&self) -> DiagnosticOwner {
        self.owner
    }

    pub(crate) fn representative_file_id(&self) -> FileId {
        self.representative_file_id
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticExternalRevision {
    owner: DiagnosticOwner,
    generation: u64,
}

impl DiagnosticExternalRevision {
    pub(crate) fn new(owner: DiagnosticOwner, generation: u64) -> Self {
        Self { owner, generation }
    }

    fn result_id_fragment(&self) -> String {
        format!("{}:{}", self.owner.result_id_fragment(), self.generation)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct DiagnosticFileRevision(u64);

impl DiagnosticFileRevision {
    pub(crate) fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    fn result_id_fragment(self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticSnapshotKey {
    snapshot_id: AnalysisSnapshotId,
    owner: DiagnosticOwner,
    readiness_revision: u64,
    diagnostics_config_revision: u64,
    target: DiagnosticTargetIdentity,
    dependency_revisions: Vec<(FileId, DiagnosticFileRevision)>,
    external_revisions: Vec<DiagnosticExternalRevision>,
}

impl DiagnosticSnapshotKey {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_snapshot_id(
        snapshot_id: AnalysisSnapshotId,
        owner: DiagnosticOwner,
        readiness_revision: u64,
        diagnostics_config_revision: u64,
        target_uri: &Url,
        target_version: Option<i32>,
        mut dependency_revisions: Vec<(FileId, DiagnosticFileRevision)>,
        mut external_revisions: Vec<DiagnosticExternalRevision>,
    ) -> Self {
        dependency_revisions.sort_unstable();
        external_revisions.sort_by_key(|revision| revision.result_id_fragment());
        Self {
            snapshot_id,
            owner,
            readiness_revision,
            diagnostics_config_revision,
            target: DiagnosticTargetIdentity::new(target_uri, target_version),
            dependency_revisions,
            external_revisions,
        }
    }

    pub(crate) fn result_id(&self) -> String {
        let dependency_revisions = self
            .dependency_revisions
            .iter()
            .map(|(file_id, revision)| {
                format!("{}:{}", file_id.index(), revision.result_id_fragment())
            })
            .collect::<Vec<_>>()
            .join(",");
        let external_revisions = self
            .external_revisions
            .iter()
            .map(DiagnosticExternalRevision::result_id_fragment)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "diag:snapshot:{}:config:{}:ready:{}:owner:{}:target:{}:deps:{}:external:{}",
            self.snapshot_id.get(),
            self.diagnostics_config_revision,
            self.readiness_revision,
            self.owner.result_id_fragment(),
            self.target.result_id_fragment(),
            dependency_revisions,
            external_revisions
        )
    }
}

#[derive(Debug, Clone)]
struct DiagnosticTargetIdentity {
    uri: String,
    version: Option<i32>,
}

impl DiagnosticTargetIdentity {
    fn new(uri: &Url, version: Option<i32>) -> Self {
        Self { uri: uri.as_str().to_owned(), version }
    }

    fn result_id_fragment(&self) -> String {
        match self.version {
            Some(version) => format!("{}:{version}", self.uri),
            None => self.uri.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_result_identity_includes_analysis_snapshot() {
        let uri = Url::parse("file:///workspace/top.sv").unwrap();
        let owner = DiagnosticOwner::File(FileId::from_raw(7));
        let first = DiagnosticSnapshotKey::new_with_snapshot_id(
            AnalysisSnapshotId::new(10),
            owner,
            0,
            0,
            &uri,
            None,
            Vec::new(),
            Vec::new(),
        )
        .result_id();
        let second = DiagnosticSnapshotKey::new_with_snapshot_id(
            AnalysisSnapshotId::new(11),
            owner,
            0,
            0,
            &uri,
            None,
            Vec::new(),
            Vec::new(),
        )
        .result_id();

        assert_ne!(first, second);
        assert!(first.contains("diag:snapshot:10:"));
        assert!(second.contains("diag:snapshot:11:"));
    }
}
