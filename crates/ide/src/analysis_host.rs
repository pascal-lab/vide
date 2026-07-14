#[cfg(test)]
use hir::base_db::project::PreprocessConfig;
use hir::base_db::{
    analysis_snapshot::{AnalysisSnapshotId, CompilationContext},
    change::Change,
    diagnostics_config::DiagnosticsConfig,
    salsa::{Durability, ParallelDatabase},
    source_db::{SourceDb, SourceRootDb},
};
use triomphe::Arc;

use crate::{analysis::Analysis, db::root_db::RootDb};

pub struct AnalysisHost {
    db: RootDb,
    snapshot_id: AnalysisSnapshotId,
}

impl AnalysisHost {
    pub fn new(lru_capacity: Option<usize>) -> AnalysisHost {
        AnalysisHost { db: RootDb::new(lru_capacity), snapshot_id: AnalysisSnapshotId::default() }
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
        self.advance_revision();
    }

    pub fn set_diagnostics_config(&mut self, config: Arc<DiagnosticsConfig>) {
        self.db.set_diagnostics_config_with_durability(config, Durability::HIGH);
        self.advance_revision();
    }

    #[cfg(test)]
    pub(crate) fn set_file_preprocess_config(
        &mut self,
        file_id: vfs::FileId,
        config: Arc<PreprocessConfig>,
    ) {
        self.db.set_file_preprocess_config_with_durability(file_id, config, Durability::LOW);
        self.advance_revision();
    }

    fn advance_revision(&mut self) {
        self.snapshot_id = self.snapshot_id.next();
    }

    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn raw_db(&self) -> &RootDb {
        &self.db
    }
}

impl AnalysisHost {
    fn compilation_contexts(&self, db: &RootDb) -> Vec<CompilationContext> {
        let mut profiles = vec![None];
        profiles.extend(db.project_config().profile_ids().into_iter().map(Some));
        profiles
            .into_iter()
            .map(|profile| db.compilation_context(profile).as_ref().clone())
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

        drop((first, second));
        host.apply_change(Change::new());
        let changed = host.make_analysis();
        assert_eq!(changed.snapshot_id().get(), 1);
    }
}
