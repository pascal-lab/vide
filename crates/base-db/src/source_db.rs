use rustc_hash::FxHashSet;
use salsa::Durability;
use syntax::parse::SyntaxTree;
use triomphe::Arc;
use utils::text_edit::SourceEditKind;
use vfs::{
    anchored_path::AnchoredPath,
    vfs::{ChangedFile, FileId},
};

use crate::source_root::{SourceRoot, SourceRootId};

pub trait FileLoader {
    fn file_text(&self, file_id: FileId) -> Arc<str>;
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

// Source code, syntax tree and project model.
// Everything else is derived from these queries.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    #[salsa::input]
    fn syntax_tree(&self, file_id: FileId) -> Option<SyntaxTree>;

    #[salsa::input]
    fn files(&self) -> FxHashSet<FileId>;
}

// `edits` = None => old syntax tree should be removed
pub fn edit_syntax_tree(
    db: &mut dyn SourceRootDb,
    changed_file: ChangedFile,
    durability: Durability,
) {
    let file_id = changed_file.file_id;

    match changed_file.source_edits() {
        Some(SourceEditKind::Full) | None => db.set_syntax_tree(file_id, None),
        Some(SourceEditKind::Edits(edits)) => {
            let syntax_tree = db.syntax_tree(file_id).expect("Initial parse expected");
            let mut tree = syntax_tree.tree().clone();

            edits.iter().for_each(|edit| tree.edit(edit));

            db.set_syntax_tree_with_durability(file_id, Some(SyntaxTree::new(tree)), durability);
        }
    }

    let text = changed_file.get_text().map_or_else(|| Arc::from(""), |s| Arc::from(s.as_str()));
    db.set_file_text_with_durability(file_id, text, durability);
}

pub fn parse_source(db: &mut dyn SourceDb, file_id: FileId) {
    // TODO: make the parser static?
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(tree_sitter_verilog::language()).unwrap();

    let old_syntax_tree = db.syntax_tree(file_id);
    let old_tree = old_syntax_tree.as_ref().map(|it| it.tree());

    let new_text = db.file_text(file_id);
    let new_tree = parser.parse(new_text.as_bytes(), old_tree).expect("tree-sitter parse error");
    let new_syntax_tree = SyntaxTree::new(new_tree);
    db.set_syntax_tree(file_id, Some(new_syntax_tree));
}

// Don't expose source roots to HIR, so extract them in a separate DB.
#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;
}
