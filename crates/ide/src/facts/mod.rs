use crate::db::root_db::RootDb;

pub(crate) mod edit;
pub(crate) mod relation;
pub(crate) mod symbol;
pub(crate) mod target;

pub(crate) use target::TargetQuery;

pub(crate) struct SemanticFacts<'db> {
    db: &'db RootDb,
}

impl<'db> SemanticFacts<'db> {
    pub(crate) fn new(db: &'db RootDb) -> Self {
        Self { db }
    }

    pub(crate) fn target_at<'tree>(
        &self,
        query: TargetQuery<'tree>,
    ) -> target::TargetResolution<'tree> {
        target::target_at(self.db, query)
    }
}
