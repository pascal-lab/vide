use crate::db::root_db::RootDb;

pub(crate) mod edit;
pub(crate) mod relation;
pub(crate) mod source_target;
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

    pub(crate) fn relations(&self) -> relation::RelationFacts<'db> {
        relation::RelationFacts::new(self.db)
    }

    #[allow(dead_code)]
    pub(crate) fn symbol(&self, id: symbol::SymbolId) -> Option<symbol::SymbolInfo> {
        id.info(self.db)
    }

    pub(crate) fn edit_plan(
        &self,
        request: edit::EditRequest<'_>,
    ) -> crate::rename::RenameResult<edit::EditPlan> {
        edit::edit_plan(self.db, request)
    }
}
