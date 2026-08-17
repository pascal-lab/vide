use std::{fmt, ops::Deref};

use base_db::{
    diagnostics_config::DiagnosticsConfig,
    project::ProjectConfig,
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceRootDb},
};
use hir_def::db::HirDefDb;
use hir_ty::db::TyDb;
use preproc_expand::db::PreprocDb;
use rustc_hash::FxHashSet;
use triomphe::Arc;
use vfs::{AnchoredPath, FileId};

use crate::db::{line_index_db::LineIndexDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb};

/// The concrete IDE Salsa database: pure, memoized computation over the input
/// sources. It holds no request-scoped cache; those live in
/// [`crate::incrementality::ProductStore`] owned by the
/// [`crate::analysis_host::AnalysisHost`].
#[salsa::db]
#[derive(Clone)]
pub struct RootDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for RootDb {}

#[salsa::db]
impl SourceDb for RootDb {}

#[salsa::db]
impl SourceRootDb for RootDb {}

#[salsa::db]
impl PreprocDb for RootDb {}

#[salsa::db]
impl HirDefDb for RootDb {}

#[salsa::db]
impl TyDb for RootDb {}

#[salsa::db]
impl LineIndexDb for RootDb {}

#[salsa::db]
impl WorkspaceSymbolIndexDb for RootDb {}

impl fmt::Debug for RootDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootDb").finish()
    }
}

impl FileLoader for RootDb {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
        let source_root = SourceRootDb::source_root(self, source_root_id);
        source_root.resolve_path(path)
    }
}

impl RootDb {
    pub fn new(lru_capacity: Option<usize>) -> RootDb {
        let mut db = RootDb { storage: salsa::Storage::default() };
        db.set_files_with_durability(Default::default(), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::HIGH);
        db.update_parse_query_lru_capacity(lru_capacity);
        db
    }

    pub fn update_parse_query_lru_capacity(&mut self, lru_capacity: Option<usize>) {
        let lru_capacity = lru_capacity.unwrap_or(DEFAULT_PARSE_LRU_CAP);
        preproc_expand::db::set_parse_lru_capacity(self, lru_capacity);
        hir_def::db::set_lru_capacity(self, lru_capacity);
    }

    /// Compute the files affected by a change through the preprocessor
    /// dependency graph: includes and dynamic includes propagate edits to
    /// every file that transitively depends on the changed sources.
    pub(crate) fn preproc_affected_files(
        &self,
        changed: impl IntoIterator<Item = FileId>,
    ) -> FxHashSet<FileId> {
        let changed = changed.into_iter().collect::<FxHashSet<_>>();
        let mut affected = changed.clone();
        let config = self.project_config();
        for profile_id in std::iter::once(None).chain(config.profile_ids().into_iter().map(Some)) {
            let plan = self.compilation_plan_for_profile(profile_id);
            let path_file_ids = self.path_file_ids();
            let mut profile_affected = plan.affected_files(changed.iter().copied());
            loop {
                let mut grew = false;
                for &includer in &plan.dynamic_include_files {
                    if profile_affected.contains(&includer) {
                        continue;
                    }
                    let Some(trace) = self.preproc_trace(includer) else {
                        continue;
                    };
                    let depends_on_affected = trace.include_edges.iter().any(|edge| {
                        trace
                            .source_buffers
                            .iter()
                            .find(|buffer| buffer.buffer_id == edge.included_buffer_id)
                            .and_then(|buffer| path_file_ids.get(&buffer.path))
                            .is_some_and(|dependency| profile_affected.contains(&dependency))
                    });
                    if depends_on_affected {
                        profile_affected.insert(includer);
                        grew = true;
                    }
                }
                let closed = plan.affected_files(profile_affected.iter().copied());
                grew |= closed.len() != profile_affected.len();
                profile_affected = closed;
                if !grew {
                    break;
                }
            }
            affected.extend(profile_affected);
        }
        affected
    }
}

/// Default memo capacity for per-file parse/HIR queries. Salsa revalidation
/// recomputes evicted memos after a revision bump, so a capacity below the
/// project's per-file working set turns incremental rebuilds into repeated
/// re-parse/re-lower work. 1024 covers small-to-medium projects without
/// pinning an unbounded number of parse trees.
pub const DEFAULT_PARSE_LRU_CAP: usize = 1024;

// RootDb is the concrete IDE database; expose the workspace query surface
// without maintaining a second set of forwarding methods.
impl Deref for RootDb {
    type Target = dyn WorkspaceSymbolIndexDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}
