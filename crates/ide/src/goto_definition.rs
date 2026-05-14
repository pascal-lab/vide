use hir::{container::InFile, semantics::Semantics};
use ide_db::root_db::RootDb;
use itertools::Itertools;
use span::{FilePosition, RangeInfo};
use syntax::{
    SyntaxNodeExt, SyntaxTokenWithParent, TokenKind,
    has_text_range::HasTextRange,
    token::{TokenKindExt, pair_token},
};

use crate::{
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
};

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let token = root.token_at_offset(offset).pick_bext_token(token_precedence)?;

    let navs = handle_ctrl_flow_kw(&sema, token).or_else(|| {
        DefinitionClass::resolve(&sema, token)?
            .origins()
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
            let tok = InFile::new(file_id, SyntaxTokenWithParent { parent, tok: pair });
            Some(vec![tok.to_nav(sema.db)])
        }
        _ => None,
    }
}

pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
