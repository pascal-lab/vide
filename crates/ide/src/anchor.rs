//! Stable anchors for facts produced by external backends.
//!
//! `Definition` is T9a: a source identity that `SourceProjection` can
//! reproject after an edit. `Instance` waits on HierPath (T10 / T9b).

use hir_def::{ast_id_map::SourceAstId, file::HirFileId};
use syntax::has_text_range::HasTextRange;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::db::root_db::RootDb;

/// A backend-independent location for an analysis fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    Definition { file: FileId, ast_id: SourceAstId },
}

pub fn project_anchor(db: &RootDb, anchor: Anchor) -> Option<TextRange> {
    match anchor {
        Anchor::Definition { file, ast_id } => db
            .source_projection(HirFileId::File(file))
            .origin(ast_id)
            .and_then(|origin| origin.focus_or_full_range()),
    }
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
    use crate::test_utils::setup;

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
        assert!(after.start() > before.start(), "insert before the name must shift the origin");
    }
}
