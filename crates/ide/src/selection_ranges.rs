use hir_semantics::semantics::Semantics;
use itertools::Itertools;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use syntax::{
    SyntaxCursorExt, SyntaxNodeExt, TokenKind, Trivia,
    has_text_range::HasTextRange,
    token::{SyntaxTokenWithParentExt, TokenKindExt},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

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
            let Some(token_range) = token.text_range() else {
                return res;
            };
            let trivias = token.trivias_with_range().collect_vec();

            // Cursor inside a trivia piece with a real range (comments,
            // whitespace). Directive trivia is zero-width and handled below.
            if let Some((range, trivia)) = trivias.iter().find(|(range, _)| range.contains(offset))
                && trivia.kind() != Trivia!["`"]
            {
                push_distinct(&mut res, *range);
            }

            // The cursor may also sit on a preprocessor directive or inside an
            // inactive `` `ifdef `` branch: directives are zero-width trivia and
            // disabled code is absent from the tree entirely, so recover their
            // ranges from the preprocessor trace.
            let mut hit = false;
            if let Some(regions) = trace_regions(db, file_id) {
                if let Some(range) = regions.directive_at(offset) {
                    push_distinct(&mut res, range);
                    hit = true;
                }
                if let Some(range) = regions.disabled_at(offset) {
                    push_distinct(&mut res, range);
                    hit = true;
                }
            }
            if !hit && res.len() == 1 {
                return res;
            }

            let Some(first_trivia) = trivias.first() else {
                return res;
            };
            let trivias_start = first_trivia.0.start();
            let range = TextRange::new(trivias_start, token_range.start());
            if !range.is_empty() && res.last() != Some(&range) {
                res.push(range);
            }
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

/// Preprocessor regions recovered from the trace: directive keyword/name
/// ranges and inactive `` `ifdef `` branch bodies. All ranges are offsets in
/// the file the tree was parsed from (the trace root buffer).
struct DirectiveRegions {
    directives: Vec<TextRange>,
    disabled: Vec<TextRange>,
}

impl DirectiveRegions {
    fn directive_at(&self, offset: TextSize) -> Option<TextRange> {
        self.directives.iter().find(|range| range.contains(offset)).copied()
    }

    fn disabled_at(&self, offset: TextSize) -> Option<TextRange> {
        self.disabled.iter().find(|range| range.contains(offset)).copied()
    }
}

fn trace_regions(db: &RootDb, file_id: FileId) -> Option<DirectiveRegions> {
    let trace = db.parse(HirFileId::File(file_id)).preprocessor_trace()?;
    let root_buffer_id = trace.root_buffer_id;

    let mut regions = DirectiveRegions { directives: Vec::new(), disabled: Vec::new() };
    for event in &trace.events {
        if let Some(directive) = &event.directive
            && let Some(range) = &directive.range
            && range.buffer_id == root_buffer_id
        {
            push_range(&mut regions.directives, &range.range);
        }
        if let Some(name) = &event.name
            && let Some(range) = &name.range
            && range.buffer_id == root_buffer_id
        {
            push_range(&mut regions.directives, &range.range);
        }
        for range in &event.disabled_ranges {
            if range.buffer_id == root_buffer_id {
                push_range(&mut regions.disabled, &range.range);
            }
        }
    }
    Some(regions)
}

fn push_range(ranges: &mut Vec<TextRange>, range: &std::ops::Range<usize>) {
    if range.start < range.end {
        ranges.push(TextRange::new(
            TextSize::from(range.start as u32),
            TextSize::from(range.end as u32),
        ));
    }
}

fn push_distinct(res: &mut Vec<TextRange>, range: TextRange) {
    if !range.is_empty() && res.last() != Some(&range) {
        res.push(range);
    }
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
    use std::fmt::Write;

    use base_db::{change::Change, source_root::SourceRoot};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::selection_ranges;
    use crate::{FilePosition, db::root_db::RootDb};

    fn db_with_file(text: &str) -> (RootDb, FileId) {
        let file_id = FileId::from_raw(0);
        let path = VfsPath::new_virtual_path("/test.sv".to_owned());

        let mut file_set = FileSet::default();
        file_set.insert(file_id, path);
        let root = SourceRoot::new_local(file_set);

        let mut change = Change::new();
        change.set_roots(vec![root]);
        change.add_changed_file(ChangedFile::create(file_id, text));

        let mut db = RootDb::new(None);
        change.apply(&mut db);
        (db, file_id)
    }

    #[test]
    fn selection_range_matrix() {
        let mut report = String::new();

        for (name, text, offset) in [
            ("empty file", "", 0),
            ("trivia-only line comment", "// hello", 3),
            ("line comment in module", "module top; // comment here\nendmodule\n", 15),
            ("directive line", "module top;\n`ifdef FOO\n  logic x;\n`endif\nendmodule\n", 14),
            (
                "inactive branch body",
                "module top;\n`ifdef FOO\n  logic x;\n`endif\nendmodule\n",
                28,
            ),
            (
                "directive continuation",
                "module top;\n`ifdef FOO\n  logic x;\n`endif\nendmodule\n",
                17,
            ),
            (
                "in begin block",
                "module top;\n  always_comb begin\n    x = 1;\n  end\nendmodule\n",
                30,
            ),
            ("at token boundary", "module top;\n  assign y = a + b;\nendmodule\n", 31),
        ] {
            let (db, file_id) = db_with_file(text);
            let ranges = selection_ranges(&db, FilePosition { file_id, offset: offset.into() });
            writeln!(&mut report, "{name}: {ranges:?}").unwrap();
        }

        insta::assert_snapshot!(report);
    }
}
