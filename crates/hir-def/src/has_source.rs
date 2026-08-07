use crate::{
    ast_id_map::SourceAstId,
    container::InFile,
    db::HirDefDb,
    def_id::DefId,
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
impl HasSource for OwnerId {
    fn source(&self, db: &dyn HirDefDb) -> Option<InFile<SourceInfo>> {
        project(db, InFile::new(self.file(db), self.ast_id(db)))
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
