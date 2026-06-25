use hir::base_db::source_db::SourceDb;
use itertools::Itertools;
use vfs::FileId;

use crate::{
    FilePosition, FileRange, RangeInfo,
    db::root_db::RootDb,
    document_symbols::{self, DocumentSymbol},
    goto_definition,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
};

pub(crate) struct RelationFacts<'db> {
    db: &'db RootDb,
}

impl<'db> RelationFacts<'db> {
    pub(crate) fn new(db: &'db RootDb) -> Self {
        Self { db }
    }

    pub(crate) fn definition_targets(
        &self,
        position: FilePosition,
    ) -> Option<RangeInfo<Vec<NavTarget>>> {
        goto_definition::goto_definition(self.db, position)
    }

    pub(crate) fn references(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Option<Vec<References>> {
        references::references(self.db, position, config)
    }

    pub(crate) fn reference_ranges(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Vec<FileRange> {
        self.references(position, config)
            .into_iter()
            .flatten()
            .flat_map(|References { refs, .. }| {
                refs.into_iter().flat_map(|(file_id, refs)| {
                    refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
                })
            })
            .unique()
            .collect()
    }

    pub(crate) fn document_symbols(&self, file_id: FileId) -> Vec<DocumentSymbol> {
        document_symbols::document_symbols(self.db, file_id)
    }

    pub(crate) fn file_ids(&self) -> Vec<FileId> {
        self.db.files().iter().copied().collect()
    }
}
