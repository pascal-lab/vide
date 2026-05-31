use hir::base_db::{project::CompilationProfileId, source_root::SourceRootId};
use lsp_types::Url;
use vfs::FileId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiagnosticCommitFreshness {
    diagnostics_revision: u64,
    readiness_revision: u64,
}

impl DiagnosticCommitFreshness {
    pub(crate) fn new(diagnostics_revision: u64, readiness_revision: u64) -> Self {
        Self { diagnostics_revision, readiness_revision }
    }

    pub(crate) fn readiness_revision(self) -> u64 {
        self.readiness_revision
    }
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
        diagnostics_revision: u64,
        target_revision: u64,
        readiness_revision: u64,
    ) -> Self {
        Self {
            commit: DiagnosticCommitFreshness::new(diagnostics_revision, readiness_revision),
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
    ExternalQihe { file: FileId },
}

impl DiagnosticOwner {
    fn result_id_fragment(self) -> String {
        match self {
            DiagnosticOwner::File(file_id) => format!("file:{}", file_id.0),
            DiagnosticOwner::SourceRoot(source_root_id) => {
                format!("source-root:{}", source_root_id.0)
            }
            DiagnosticOwner::CompilationProfile(profile_id) => {
                format!("compilation-profile:{}", profile_id.0)
            }
            DiagnosticOwner::ExternalQihe { file } => format!("external-qihe:{}", file.0),
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
    owner: DiagnosticOwner,
    readiness_revision: u64,
    diagnostics_config_revision: u64,
    target: DiagnosticTargetIdentity,
    dependency_revisions: Vec<(FileId, DiagnosticFileRevision)>,
    external_revisions: Vec<DiagnosticExternalRevision>,
}

impl DiagnosticSnapshotKey {
    pub(crate) fn new(
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
            .map(|(file_id, revision)| format!("{}:{}", file_id.0, revision.result_id_fragment()))
            .collect::<Vec<_>>()
            .join(",");
        let external_revisions = self
            .external_revisions
            .iter()
            .map(DiagnosticExternalRevision::result_id_fragment)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "diag:config:{}:ready:{}:owner:{}:target:{}:deps:{}:external:{}",
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
