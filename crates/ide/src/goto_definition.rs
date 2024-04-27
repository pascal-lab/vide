use base_db::source_db::{SourceDb, SourceRootDb};
use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use span::{FilePosition, RangeInfo};
use syntax::{
    ast::AstNode,
    syntax_kind,
    treesit_ext::{pick_best_token, token_at_offset},
};

use crate::navigation_target::NavTarget;

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);
    let tree = db.syntax_tree(file_id).unwrap();
    let token = pick_best_token(token_at_offset(file.syntax(), offset), token_precedence)?;
    todo!()
}

fn token_precedence(kind: syntax_kind::SyntaxKindId) -> usize {
    match kind {
        syntax_kind::IDENTIFIER => 4,
        _ => 1,
    }
}
