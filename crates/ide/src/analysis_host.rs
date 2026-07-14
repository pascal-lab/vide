use hir::base_db::{
    analysis_snapshot::{AnalysisSnapshotId, CompilationContext},
    change::Change,
    project::CompilationProfileId,
    salsa::ParallelDatabase,
    source_db::{SourceDb, SourceFileKind, SourceRootDb},
};
use triomphe::Arc;

use crate::{analysis::Analysis, db::root_db::RootDb};

pub struct AnalysisHost {
    db: RootDb,
    snapshot_id: AnalysisSnapshotId,
    document_revision: u64,
}

impl AnalysisHost {
    pub fn new(lru_capacity: Option<usize>) -> AnalysisHost {
        AnalysisHost {
            db: RootDb::new(lru_capacity),
            snapshot_id: AnalysisSnapshotId::default(),
            document_revision: 0,
        }
    }

    pub fn make_analysis(&self) -> Analysis {
        let db = self.db.snapshot();
        let compilation_contexts = self.compilation_contexts(&db);
        Analysis {
            db,
            snapshot_id: self.snapshot_id,
            compilation_contexts: Arc::from(compilation_contexts),
        }
    }

    pub fn apply_change(&mut self, change: Change) {
        self.db.apply_change(change);
        self.document_revision = self.document_revision.saturating_add(1);
        self.snapshot_id = self.snapshot_id.next();
    }

    pub fn mark_changed(&mut self) {
        self.document_revision = self.document_revision.saturating_add(1);
        self.snapshot_id = self.snapshot_id.next();
    }

    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn document_revision(&self) -> u64 {
        self.document_revision
    }

    pub fn raw_db(&self) -> &RootDb {
        &self.db
    }

    pub fn raw_db_mut(&mut self) -> &mut RootDb {
        &mut self.db
    }
}

impl AnalysisHost {
    fn compilation_contexts(&self, db: &RootDb) -> Vec<CompilationContext> {
        let mut profiles = db.project_config().profile_ids();
        profiles.insert(0, CompilationProfileId(u32::MAX));

        profiles
            .into_iter()
            .map(|profile| {
                let profile = (profile != CompilationProfileId(u32::MAX)).then_some(profile);
                let plan = db.compilation_plan_for_profile(profile);
                let library_maps = plan
                    .roots
                    .iter()
                    .copied()
                    .filter(|file_id| matches!(db.file_kind(*file_id), SourceFileKind::LibraryMap))
                    .collect::<Vec<_>>();
                CompilationContext::new(
                    profile,
                    plan.roots.clone(),
                    plan.include_dirs.clone(),
                    plan.predefines.clone(),
                    library_maps,
                    plan.top_modules.clone(),
                    self.document_revision,
                )
            })
            .collect()
    }
}

impl Default for AnalysisHost {
    fn default() -> AnalysisHost {
        AnalysisHost::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_views_share_one_snapshot_identity() {
        let mut host = AnalysisHost::default();
        let first = host.make_analysis();
        let second = host.make_analysis();

        assert_eq!(first.snapshot_id(), second.snapshot_id());
        assert_eq!(first.snapshot_id().get(), 0);
        assert_eq!(first.compilation_contexts()[0].document_revision, 0);

        drop((first, second));
        host.apply_change(Change::new());
        let changed = host.make_analysis();
        assert_eq!(changed.snapshot_id().get(), 1);
        assert_eq!(changed.compilation_contexts()[0].document_revision, 1);
    }
}
