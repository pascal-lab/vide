use crate::{
    ast_id_map::SourceAstId,
    container::{InFile, ScopeId, SubroutineScope},
    db::HirDefDb,
    def_id::{DefId, subroutine_src},
    module::{ModuleId, generate::GenerateBlockId},
    owner::OwnerId,
    source_map::SourceInfo,
    symbol::{DefOrigin, DefOriginLoc},
};

pub trait HasSource {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>>;
}

fn project(db: &dyn HirDefDb, source: InFile<SourceAstId>) -> Option<InFile<SourceInfo>> {
    let origin = db.source_projection(source.file_id).origin(source.value)?;
    Some(InFile::new(source.file_id, SourceInfo::from_origin(origin)?))
}

impl HasSource for ModuleId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        let InFile { file_id, value } = *self;
        project(db, InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
    }
}

impl HasSource for OwnerId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        project(db, InFile::new(self.file(db), self.ast_id(db)))
    }
}

impl HasSource for GenerateBlockId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        project(db, self.loc().src)
    }
}

impl HasSource for SubroutineScope {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        project(db, subroutine_src(db, self.clone())?)
    }
}
impl HasSource for ScopeId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        match self.clone() {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => module_id.source(db),
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.source(db),
            ScopeId::Subroutine(subroutine) => subroutine.source(db),
            ScopeId::Owner(owner) => owner.source(db),
            ScopeId::ClockingBlock(clocking_block) => {
                DefOrigin::new(db, DefOriginLoc::ClockingBlock(clocking_block)).source(db)
            }
            ScopeId::Checker(checker) => {
                DefOrigin::new(db, DefOriginLoc::Checker(checker)).source(db)
            }
            ScopeId::Covergroup(covergroup) => {
                DefOrigin::new(db, DefOriginLoc::Covergroup(covergroup)).source(db)
            }
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
        DefOrigin::new(db, self.clone()).source(db)
    }
}

impl HasSource for DefId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        self.primary_origin(db).source(db)
    }
}
