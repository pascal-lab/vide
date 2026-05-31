use hir::base_db::{change::Change, salsa::ParallelDatabase};

use crate::{analysis::Analysis, db::root_db::RootDb};

pub struct AnalysisHost {
    db: RootDb,
}

impl AnalysisHost {
    pub fn new(lru_capacity: Option<usize>) -> AnalysisHost {
        AnalysisHost { db: RootDb::new(lru_capacity) }
    }

    pub fn make_analysis(&self) -> Analysis {
        Analysis { db: self.db.snapshot() }
    }

    pub fn apply_change(&mut self, change: Change) {
        self.db.apply_change(change);
    }

    pub fn raw_db(&self) -> &RootDb {
        &self.db
    }

    pub fn raw_db_mut(&mut self) -> &mut RootDb {
        &mut self.db
    }
}

impl Default for AnalysisHost {
    fn default() -> AnalysisHost {
        AnalysisHost::new(None)
    }
}
