//! Stable anchors for facts produced by external backends.
//!
//! `Definition` is T9a: a source identity that `SourceProjection` can
//! reproject after an edit. `Instance` is T9b: an elaborated hierarchical
//! path; the live compilation answers where it sits in the current source.

use hir_def::{ast_id_map::SourceAstId, file::HirFileId};
use syntax::has_text_range::HasTextRange;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{db::root_db::RootDb, hier::HierPath};

/// A backend-independent location for an analysis fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Anchor {
    Definition { file: FileId, ast_id: SourceAstId },
    Instance { path: HierPath },
}

/// Current source span of an [`Anchor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedAnchor {
    pub file: FileId,
    pub range: TextRange,
}

pub fn project_definition(db: &RootDb, file: FileId, ast_id: SourceAstId) -> Option<TextRange> {
    db.source_projection(HirFileId::File(file))
        .origin(ast_id)
        .and_then(|origin| origin.focus_or_full_range())
}

pub(crate) fn project(
    ctx: &crate::analysis::AnalysisContext<'_>,
    anchor: &Anchor,
) -> Option<ProjectedAnchor> {
    match anchor {
        Anchor::Definition { file, ast_id } => project_definition(ctx.db, *file, *ast_id)
            .map(|range| ProjectedAnchor { file: *file, range }),
        Anchor::Instance { path } => project_instance(ctx, path),
    }
}

fn project_instance(
    ctx: &crate::analysis::AnalysisContext<'_>,
    path: &HierPath,
) -> Option<ProjectedAnchor> {
    use crate::elaboration::ElabResult;

    let profiles = {
        let ids = ctx.db.project_config().profile_ids();
        if ids.is_empty() { vec![None] } else { ids.into_iter().map(Some).collect::<Vec<_>>() }
    };
    for profile in profiles {
        let rows = match ctx.elab.list_instances(ctx.db, ctx.revision, profile) {
            ElabResult::Ready(Some(rows)) => rows,
            _ => continue,
        };
        let Some(row) = rows.iter().find(|row| row.path == path.as_str()) else {
            continue;
        };
        let file = file_id_for_slang_path(ctx.db, &row.file);
        let tail = path.as_str().rsplit('.').next().unwrap_or(path.as_str());
        let name_len = tail.find('[').unwrap_or(tail.len());
        let start = utils::line_index::TextSize::from(row.offset as u32);
        let range =
            TextRange::new(start, start + utils::line_index::TextSize::from(name_len as u32));
        return Some(ProjectedAnchor { file, range });
    }
    None
}

fn file_id_for_slang_path(db: &RootDb, slang_file: &str) -> FileId {
    preproc_expand::db::PreprocDb::path_file_ids(db).get(slang_file).unwrap_or_else(|| {
        panic!("elaboration reported a buffer path that was not assigned: {slang_file}")
    })
}

/// Innermost syntax node covering `range`, identified by [`SourceAstId`].
pub fn ast_id_at_range(db: &RootDb, file: FileId, range: TextRange) -> Option<SourceAstId> {
    let hir_file = HirFileId::File(file);
    let tree = db.parse(hir_file);
    let map = db.ast_id_map(hir_file);
    let mut best: Option<(TextSizeLen, SourceAstId)> = None;
    for event in tree.root().node_preorder() {
        let syntax::WalkEvent::Enter(node) = event else {
            continue;
        };
        let Some(node_range) = node.text_range() else {
            continue;
        };
        if !covers(node_range, range) {
            continue;
        }
        let Some(id) = map.id_of_node(node) else {
            continue;
        };
        let len = node_range.len();
        if best.map(|(best_len, _)| len < best_len).unwrap_or(true) {
            best = Some((len, id));
        }
    }
    best.map(|(_, id)| id)
}

type TextSizeLen = utils::line_index::TextSize;

fn covers(outer: TextRange, inner: TextRange) -> bool {
    outer.start() <= inner.start() && inner.end() <= outer.end()
}

#[cfg(test)]
mod tests {
    use base_db::change::Change;
    use vfs::ChangedFile;

    use super::*;
    use crate::{hier::HierPath, test_utils::setup};

    #[test]
    fn a_definition_anchor_survives_an_insert_before_it() {
        let src = "module foo; endmodule\n";
        let (mut host, file_id) = setup(src);
        let offset = src.find("foo").expect("name");
        let range = TextRange::new(
            utils::line_index::TextSize::from(offset as u32),
            utils::line_index::TextSize::from((offset + 3) as u32),
        );
        let (ast_id, before) = {
            let analysis = host.make_analysis();
            let ast_id = analysis.ast_id_at_range(file_id, range).unwrap().expect("module name id");
            let before = analysis
                .project_anchor(Anchor::Definition { file: file_id, ast_id })
                .unwrap()
                .expect("origin");
            (ast_id, before)
        };

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(file_id, format!("// header\n{src}").as_str()));
        host.apply_change(change);

        let after = host
            .make_analysis()
            .project_anchor(Anchor::Definition { file: file_id, ast_id })
            .unwrap()
            .expect("reprojected origin");
        assert!(
            after.range.start() > before.range.start(),
            "insert before the name must shift the origin"
        );
    }

    #[test]
    fn an_instance_anchor_tracks_the_instantiation_site() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let (mut host, file_id) = crate::test_utils::setup_with_path(src, "/top.sv");
        let path = {
            let ctx = host.ctx();
            let rows = match ctx.elab.list_instances(
                ctx.db,
                ctx.revision,
                ctx.db.file_compilation_profile(file_id),
            ) {
                crate::elaboration::ElabResult::Ready(Some(rows)) => rows,
                other => panic!("expected instances, got {other:?}"),
            };
            rows.into_iter()
                .find(|row| row.path.contains("u0"))
                .map(|row| HierPath::new(row.path))
                .expect("u0")
        };
        let before = host
            .make_analysis()
            .project_anchor(Anchor::Instance { path: path.clone() })
            .unwrap()
            .expect("instance origin");
        assert_eq!(before.file, file_id);
        assert_eq!(usize::from(before.range.start()), src.find("u0").expect("u0"),);

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(file_id, format!("// header\n{src}").as_str()));
        host.apply_change(change);
        let after = host
            .make_analysis()
            .project_anchor(Anchor::Instance { path })
            .unwrap()
            .expect("reprojected instance");
        assert!(after.range.start() > before.range.start());
    }
}
