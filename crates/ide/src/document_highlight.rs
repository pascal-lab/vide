use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Either;
use line_index::TextRange;
use span::FilePosition;
use syntax::{
    SyntaxNodeExt, SyntaxToken, SyntaxTokenWithParent, TokenKind,
    ast::AstNode,
    has_text_range::HasTextRange,
    token::{is_pair_token, pair_token},
};

use crate::references::ReferenceCategory;

#[derive(Debug, Clone)]
pub struct DocumentHighlight {
    pub range: TextRange,
    pub category: ReferenceCategory,
}

impl DocumentHighlight {
    pub fn new(range: TextRange) -> Self {
        Self { range, category: ReferenceCategory::empty() }
    }
}

pub(crate) fn document_highlight(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<Vec<DocumentHighlight>> {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);

    let token = file.syntax().token_at_offset(offset).pick_bext_token(token_precedence)?;

    handle_ctrl_flow_kw(&sema, token)
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if is_pair_token(kind) => 4,
        _ => 1,
    }
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<'_, RootDb>,
    tok_with_parent @ SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
) -> Option<Vec<DocumentHighlight>> {
    let file_id = sema.find_file(parent);
    let mut res = vec![DocumentHighlight::new(tok.text_range().unwrap())];

    if let Some(pair) = pair_token(tok_with_parent) {
        let pair: SyntaxToken = pair.either_into();
        res.push(DocumentHighlight::new(pair.text_range().unwrap()));
    }

    Some(res)
}
