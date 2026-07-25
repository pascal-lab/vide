use base_db::intern::Lookup;

use crate::{
    block::BlockId,
    container::{InFile, ScopeId, SubroutineScope},
    db::HirDefDb,
    def_id::{DefId, subroutine_src},
    module::{ModuleId, generate::GenerateBlockId},
    source_map::{IsNamedSrc, SourceInfo},
    symbol::{DefOrigin, DefOriginLoc},
};

pub trait HasSource {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>>;
}

fn named_source(
    file_id: preproc_expand::file::HirFileId,
    src: impl IsNamedSrc,
) -> InFile<SourceInfo> {
    InFile::new(file_id, SourceInfo::named(src))
}

impl HasSource for ModuleId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value } = *self;
        let lowered = db.hir_file_with_source_map(file_id);
        Some(named_source(file_id, lowered.source(value)?))
    }
}

impl HasSource for BlockId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value } = self.lookup(db).src;
        Some(named_source(file_id, value))
    }
}

impl HasSource for GenerateBlockId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value } = self.lookup(db).src;
        Some(named_source(file_id, value))
    }
}

impl HasSource for SubroutineScope {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value } = subroutine_src(db, *self)?;
        Some(named_source(file_id, value))
    }
}
impl HasSource for ScopeId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        match *self {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => module_id.source(db),
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.source(db),
            ScopeId::Block(block_id) => block_id.source(db),
            ScopeId::Subroutine(subroutine) => subroutine.source(db),
            ScopeId::ClockingBlock(clocking_block) => DefOrigin::new(db, clocking_block).source(db),
            ScopeId::Checker(checker) => DefOrigin::new(db, checker).source(db),
            ScopeId::Covergroup(covergroup) => DefOrigin::new(db, covergroup).source(db),
        }
    }
}

impl HasSource for DefOrigin {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value: full_range } = self.range(db)?;
        let focus_range =
            self.name_range(db).filter(|focus| focus.file_id == file_id).map(|focus| focus.value);
        Some(InFile::new(file_id, SourceInfo::from_ranges(full_range, focus_range)))
    }
}

impl HasSource for DefOriginLoc {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        DefOrigin::new(db, *self).source(db)
    }
}

impl HasSource for DefId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        self.primary_origin(db).source(db)
    }
}
