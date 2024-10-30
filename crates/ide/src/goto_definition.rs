use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::{Either, Itertools};
use span::{FilePosition, RangeInfo};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_name::HasName,
    has_text_range::HasTextRange,
    match_ast, support,
    token::{is_pair_token, pair_token},
};

use crate::{
    SymbolKind,
    definitions::Definition,
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);
    let token = file.syntax().token_at_offset(offset).pick_bext_token(token_precedence)?;

    let navs = handle_ctrl_flow_kw(&sema, token).or_else(|| {
        Definition::resolution(&sema, token)?
            .into_iter()
            .map(|def| def.to_nav(db))
            .unique()
            .collect_vec()
            .into()
    })?;

    Some(RangeInfo::new(token.text_range()?, navs))
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<RootDb>,
    tok_with_parent @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let kind = tok.kind();
    let file_id = sema.find_file(parent);

    if let Some(pair) = pair_token(tok_with_parent) {
        let tok = match pair {
            Either::Left(pair) => pair,
            Either::Right(_) => tok,
        };
        // TODO: name and container_name
        let nav = NavTarget {
            file_id: file_id.file_id(),
            full_range: parent.text_range().unwrap(),
            focus_range: tok.text_range(),
            name: None,
            kind: Some(SymbolKind::from_node(parent)),
            container_name: None,
            description: None,
        };

        return Some(vec![nav]);
    }

    None
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => 4,
        _ if is_pair_token(kind) => 4,
        _ => 1,
    }
}
