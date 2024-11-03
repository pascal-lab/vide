use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use span::{FilePosition, RangeInfo};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    ast::AstNode,
    has_text_range::HasTextRange,
    token::{TokenKindExt, pair_token},
};

use crate::{
    SymbolKind,
    definitions::DefinitionClass,
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
        DefinitionClass::resolve(&sema, token)?
            .sources()
            .into_iter()
            .unique()
            .map(|def| def.to_nav(db))
            .collect_vec()
            .into()
    })?;

    Some(RangeInfo::new(token.text_range()?, navs))
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<RootDb>,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let file_id = sema.find_file(parent);
    let kind = tok.kind();

    match kind {
        _ if let Some(pair) = pair_token(tp) => {
            let pair = pair.either(|pair| pair, |_| tok);

            // TODO: name and container_name
            let nav = NavTarget {
                file_id: file_id.file_id(),
                full_range: parent.text_range().unwrap(),
                focus_range: pair.text_range(),
                name: None,
                kind: Some(SymbolKind::from_node(parent)),
                container_name: None,
                description: None,
            };

            Some(vec![nav])
        }
        _ => None,
    }
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
