use hir::semantics::Semantics;
use itertools::Itertools;
use syntax::{
    SyntaxCursorExt, SyntaxNodeExt, TokenKind,
    has_text_range::HasTextRange,
    token::{SyntaxTokenWithParentExt, TokenKindExt},
};
use utils::line_index::TextRange;

use crate::{FilePosition, db::root_db::RootDb};

pub(crate) fn selection_ranges(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Vec<TextRange> {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
        return vec![TextRange::empty(offset)];
    };

    // LSP expects one selection tree per requested position. Start with the
    // cursor range, then add slang trivia/token/node ranges when they exist.
    let mut res = vec![TextRange::empty(offset)];

    let mut cursor = root.walk();

    let trivias_start = match root.token_at_offset(offset).pick_bext_token(token_precedence) {
        Some(token) => {
            let Some(token_range) = token.text_range() else {
                return res;
            };
            if !cursor.goto_first_tok_after_or_last(token_range.start()) {
                return res;
            }
            None
        }
        None => {
            if !cursor.goto_first_tok_after_or_last(offset) {
                return res;
            }
            let Some(token) = cursor.to_tok_with_parent() else {
                return res;
            };
            let trivias = token.trivias_with_range().collect_vec();
            let Some(range) = trivias.iter().find(|(range, _)| range.contains(offset)) else {
                return res;
            };
            res.push(range.0);

            let (Some(first_trivia), Some(token_range)) = (trivias.first(), token.text_range())
            else {
                return res;
            };
            let trivias_start = first_trivia.0.start();
            res.push(TextRange::new(trivias_start, token_range.start()));
            Some(trivias_start)
        }
    };

    let mut push_to_res = |mut range: TextRange| {
        if let Some(trivias_start) = trivias_start
            && trivias_start < range.start()
        {
            range = TextRange::new(trivias_start, range.end());
        }
        if !range.is_empty() && res.last() != Some(&range) {
            res.push(range);
        }
    };

    let Some(token) = cursor.to_tok_with_parent() else {
        return res;
    };
    let Some(mut range) = token.text_range() else {
        return res;
    };
    push_to_res(range);

    while cursor.goto_parent() {
        if let Some(new_range) = cursor.to_elem().text_range()
            && new_range != range
        {
            range = new_range;
            push_to_res(range);
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

#[cfg(test)]
mod tests {
    use hir::base_db::{change::Change, source_root::SourceRoot};
    use triomphe::Arc;
    use utils::{
        line_index::{TextRange, TextSize},
        lines::LineEnding,
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::selection_ranges;
    use crate::{FilePosition, db::root_db::RootDb};

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
        });

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    #[test]
    fn empty_file_keeps_cursor_selection_range() {
        let (db, file_id) = db_with_file("");
        let ranges = selection_ranges(&db, FilePosition { file_id, offset: 0.into() });

        assert_eq!(ranges.first(), Some(&TextRange::empty(0.into())));
    }

    #[test]
    fn trivia_only_file_keeps_cursor_and_comment_ranges() {
        let text = "// hello";
        let (db, file_id) = db_with_file(text);
        let ranges = selection_ranges(&db, FilePosition { file_id, offset: 3.into() });

        assert_eq!(ranges.first(), Some(&TextRange::empty(3.into())));
        assert!(ranges.contains(&TextRange::new(0.into(), TextSize::of(text))));
    }
}
