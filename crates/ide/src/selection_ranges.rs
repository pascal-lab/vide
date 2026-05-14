use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use itertools::Itertools;
use span::FilePosition;
use syntax::{
    SyntaxCursorExt, SyntaxNodeExt, TokenKind,
    has_text_range::HasTextRange,
    token::{SyntaxTokenExt, TokenKindExt},
};
use utils::line_index::TextRange;

pub(crate) fn selection_ranges(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Vec<TextRange> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);

    let mut res = Vec::new();

    let mut cursor = root.walk();

    let trivias_start = match root.token_at_offset(offset).pick_bext_token(token_precedence) {
        Some(token) => {
            cursor.goto_first_tok_after_or_last(token.text_range().unwrap().start());
            None
        }
        None => {
            cursor.goto_first_tok_after_or_last(offset);
            let token = cursor.to_token().unwrap();
            let trivias = token.trivias_with_range().collect_vec();
            let range = trivias.iter().find(|(range, _)| range.contains(offset)).unwrap();
            res.push(range.0);

            let trivias_start = trivias[0].0.start();
            res.push(TextRange::new(trivias_start, token.text_range().unwrap().start()));
            Some(trivias_start)
        }
    };

    let mut push_to_res = |mut range: TextRange| {
        if let Some(trivias_start) = trivias_start
            && trivias_start < range.start()
        {
            range = TextRange::new(trivias_start, range.end());
        }
        if !range.is_empty() {
            res.push(range);
        }
    };

    let mut range = cursor.to_token().unwrap().text_range().unwrap();
    push_to_res(range);

    while cursor.goto_parent() {
        let new_range = cursor.to_elem().text_range().unwrap();
        if new_range != range {
            push_to_res(range);
            range = new_range
        }
    }

    res
}

fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_literal() => 3,
        _ => 1,
    }
}
