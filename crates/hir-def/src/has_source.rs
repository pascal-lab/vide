use base_db::intern::Lookup;

use crate::{
    block::{BlockId, BlockSrc},
    container::InFile,
    db::HirDefDb,
    module::{ModuleId, ModuleSrc},
    source_map::IsSrc,
};

pub trait HasSource {
    type AstPtr: IsSrc;

    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<Self::AstPtr>>;
}

impl HasSource for ModuleId {
    type AstPtr = ModuleSrc;

    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<ModuleSrc>> {
        let InFile { file_id, value } = *self;
        let lowered = db.hir_file_with_source_map(file_id);
        Some(self.with_value(lowered.source(value)?))
    }
}

impl HasSource for BlockId {
    type AstPtr = BlockSrc;

    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<BlockSrc>> {
        Some(self.lookup(db).src)
    }
}
