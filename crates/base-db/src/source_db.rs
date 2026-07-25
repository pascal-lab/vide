use rustc_hash::FxHashSet;
use triomphe::Arc;
use vfs::{AnchoredPath, FileId};
pub use workspace_model::source_db::SourceFileKind;

use crate::{
    diagnostics_config::DiagnosticsConfig,
    project::{CompilationProfileId, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};

pub trait FileLoader {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

/// Ground-state source files and project configuration.
///
/// Derived preprocessing, lowering, and type-system queries belong to higher
/// database layers.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn file_kind(&self, file_id: FileId) -> SourceFileKind;

    #[salsa::input]
    fn file_path(&self, file_id: FileId) -> Option<utils::paths::AbsPathBuf>;

    #[salsa::input]
    fn files(&self) -> Box<FxHashSet<FileId>>;

    #[salsa::input]
    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig>;

    #[salsa::input]
    fn project_config(&self) -> Arc<ProjectConfig>;
}

#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;

    fn file_compilation_profile(&self, file_id: FileId) -> Option<CompilationProfileId>;
    fn file_is_project_ignored(&self, file_id: FileId) -> bool;
}

fn file_compilation_profile(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Option<CompilationProfileId> {
    let source_root_id = db.source_root_id(file_id);
    let project_config = db.project_config();
    let profile_id = project_config.profile_for_root(source_root_id);
    let source_root = db.source_root(source_root_id);
    if profile_id.is_none() && source_root.role().reports_missing_profile() {
        tracing::debug!(
            ?file_id,
            ?source_root_id,
            root_profile_count = project_config.root_profile_count(),
            "file has no compilation profile",
        );
    }
    profile_id
}

fn file_is_project_ignored(db: &dyn SourceRootDb, file_id: FileId) -> bool {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).is_ignored()
}
