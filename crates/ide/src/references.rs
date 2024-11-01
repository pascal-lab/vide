use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use line_index::TextRange;
use nohash_hasher::IntMap;
use span::FilePosition;
use syntax::{
    SyntaxNodeExt, SyntaxToken, SyntaxTokenWithParent, TokenKind,
    ast::AstNode,
    has_text_range::HasTextRange,
    token::{TokenKindExt, pair_token},
};
use vfs::FileId;

use crate::navigation_target::NavTarget;

bitflags::bitflags! {
    #[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
    pub struct ReferenceCategory: u8 {
        const WRITE = 1 << 0;
        const READ = 1 << 1;
    }
}

#[derive(Debug, Clone)]
pub struct References {
    pub def: Option<NavTarget>,
    pub refs: IntMap<FileId, Vec<(TextRange, ReferenceCategory)>>,
}

pub(crate) fn references(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<Vec<References>> {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);

    let token = file.syntax().token_at_offset(offset).pick_bext_token(token_precedence)?;

    handle_ctrl_flow_kw(&sema, token).or_else(|| None)
}

pub(crate) fn handle_ctrl_flow_kw(
    sema: &Semantics<'_, RootDb>,
    tok_with_parent @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Vec<References>> {
    let file_id = sema.find_file(parent);
    let kind = tok.kind();
    let mut refs = vec![];

    match kind {
        _ if let Some(pair) = pair_token(tok_with_parent) => {
            let pair: SyntaxToken = pair.either_into();
            refs.push((tok.text_range().unwrap(), ReferenceCategory::empty()));
            refs.push((pair.text_range().unwrap(), ReferenceCategory::empty()));
        }
        _ => return None,
    }

    Some(vec![References { def: None, refs: IntMap::from_iter([(file_id.file_id(), refs)]) }])
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
