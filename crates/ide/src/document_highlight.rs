use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use line_index::TextRange;
use span::FilePosition;
use syntax::{ast::AstNode, token::TokenKindExt, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind};

use crate::references::{self, ReferenceCategory};

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
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<'_, RootDb>,
    tok_with_parent: SyntaxTokenWithParent,
) -> Option<Vec<DocumentHighlight>> {
    let cur_file_id = sema.find_file(tok_with_parent.parent).file_id();
    let highlights = references::handle_ctrl_flow_kw(sema, tok_with_parent)?
        .into_iter()
        .filter_map(|mut r| r.refs.remove(&cur_file_id))
        .flatten()
        .map(|(range, category)| DocumentHighlight { range, category })
        .collect();
    Some(highlights)
}
