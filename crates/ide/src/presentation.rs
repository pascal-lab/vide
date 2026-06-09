use hir::{
    base_db::source_db::SourceRootDb,
    container::InFile,
    file::HirFileId,
    preproc::resolve_source_presentation_anchor,
    source_map::{SourcePresentation, SourcePresentationAnchor},
};
use utils::line_index::TextRange;
use vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationNavRange {
    pub file_id: FileId,
    pub full_range: TextRange,
    pub focus_range: Option<TextRange>,
}

pub(crate) fn presentation_full_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<TextRange> {
    same_file_range(file_id, presentation_file_range(db, file_id, presentation.full)?)
}

pub(crate) fn presentation_name_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<TextRange> {
    same_file_range(file_id, presentation_file_range(db, file_id, presentation.name?)?)
}

pub(crate) fn presentation_full_file_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<InFile<TextRange>> {
    presentation_file_range(db, file_id, presentation.full)
}

pub(crate) fn presentation_name_file_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<InFile<TextRange>> {
    presentation_file_range(db, file_id, presentation.name?)
}

pub(crate) fn presentation_name_or_full_file_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<InFile<TextRange>> {
    presentation_name_file_range(db, file_id, presentation)
        .or_else(|| presentation_full_file_range(db, file_id, presentation))
}

pub(crate) fn presentation_nav_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    presentation: &SourcePresentation,
) -> Option<PresentationNavRange> {
    let full = presentation_full_file_range(db, file_id, presentation);
    let focus = presentation_name_file_range(db, file_id, presentation);
    match (full, focus) {
        (Some(full), Some(focus)) if full.file_id == focus.file_id => Some(PresentationNavRange {
            file_id: full.file_id.file_id(),
            full_range: full.value,
            focus_range: Some(focus.value),
        }),
        (Some(full), None) => Some(PresentationNavRange {
            file_id: full.file_id.file_id(),
            full_range: full.value,
            focus_range: None,
        }),
        (None, Some(focus)) => Some(PresentationNavRange {
            file_id: focus.file_id.file_id(),
            full_range: focus.value,
            focus_range: Some(focus.value),
        }),
        _ => None,
    }
}

fn presentation_file_range(
    db: &dyn SourceRootDb,
    file_id: HirFileId,
    anchor: SourcePresentationAnchor,
) -> Option<InFile<TextRange>> {
    let target =
        resolve_source_presentation_anchor(db, file_id.file_id(), anchor).ok()?.available()?;
    Some(InFile::new(HirFileId(target.file_id), target.range))
}

fn same_file_range(file_id: HirFileId, range: InFile<TextRange>) -> Option<TextRange> {
    (range.file_id == file_id).then_some(range.value)
}
