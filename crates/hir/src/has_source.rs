use utils::get::Get;

use crate::{
    base_db::intern::Lookup,
    container::InFile,
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockSrc},
        module::{ModuleId, ModuleSrc},
    },
    source_map::IsSrc,
};

pub trait HasSource {
    type AstPtr: IsSrc;

    fn source(&self, db: &dyn HirDb) -> Option<InFile<Self::AstPtr>>;
}

impl HasSource for ModuleId {
    type AstPtr = ModuleSrc;

    fn source(&self, db: &dyn HirDb) -> Option<InFile<ModuleSrc>> {
        let InFile { file_id, value } = *self;
        let (_, file_source_map) = db.hir_file_with_source_map(file_id);
        Some(self.with_value(file_source_map.get(value)?))
    }
}

impl HasSource for BlockId {
    type AstPtr = BlockSrc;

    fn source(&self, db: &dyn HirDb) -> Option<InFile<BlockSrc>> {
        Some(self.lookup(db).src)
    }
}
