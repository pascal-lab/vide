use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use smallvec::SmallVec;
use span::{FilePosition, RangeInfo};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    match_ast,
    token::{TokenKindExt, pair_token},
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
        resolution(&sema, token)?
            .into_iter()
            .map(|def| def.to_nav(db))
            .unique()
            .collect_vec()
            .into()
    })?;

    Some(RangeInfo::new(token.text_range()?, navs))
}

pub(crate) fn resolution(
    sema: &Semantics<'_, RootDb>,
    tp @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<SmallVec<[Definition; 3]>> {
    if !matches!(tok.kind(), TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER) {
        return None;
    }

    let res = match_ast! { parent in
        ast::MemberAccessExpression => unimplemented!(),
        ast::ScopedName => unimplemented!(),
        ast::NamedPortConnection[it] if it.name() == Some(tok) => {
            let mut res = SmallVec::new();
            if let Some(port_conn_res) = sema.resolve_port_conn_name(it) {
                res.extend(Definition::from_pathres(port_conn_res).into_iter());
            }

            if it.open_paren().is_none() && it.close_paren().is_none()
            && let Some(in_cont_res) = sema.resolve_ident_in_cont(tp) {
                res.extend(Definition::from_pathres(in_cont_res).into_iter());
            };
            return Some(res);
        },
        _ => sema.resolve_ident_in_cont(tp),
    }?;

    Some(Definition::from_pathres(res))
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
