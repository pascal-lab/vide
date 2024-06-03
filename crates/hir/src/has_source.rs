use base_db::intern::Lookup;
use syntax::ast::ptr;

use crate::{
    container::InFile,
    db::HirDb,
    hir_def::{
        block::{
            block_src::{BlockSrc, LocalBlockSrc},
            BlockId, BlockLoc,
        },
        ModuleId, ModuleSrc,
    },
};

pub trait HasSource {
    type AstPtr;

    fn source(&self, db: &dyn HirDb) -> Option<InFile<Self::AstPtr>>;
}

impl HasSource for ModuleId {
    type AstPtr = ptr::ModuleDeclarationPtr;

    fn source(&self, db: &dyn HirDb) -> Option<ModuleSrc> {
        let InFile { file_id, value } = &self;
        let (_, file_source_map) = db.hir_file_with_source_map(*file_id);
        file_source_map.modules.hir2src.get(*value).map(|it| it.clone())
    }
}

impl HasSource for BlockId {
    type AstPtr = LocalBlockSrc;

    fn source(&self, db: &dyn HirDb) -> Option<BlockSrc> {
        let BlockLoc { block_src, .. } = self.lookup(db);
        Some(block_src)
    }
}
