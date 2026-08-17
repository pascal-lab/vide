use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::{
    SyntaxElement, TokenKind, WalkEvent, has_text_range::HasTextRange, ptr::SyntaxTokenPtr,
    token::TokenKindExt,
};
use vfs::FileId;

use super::{FileNameIndex, NameOccurrence};
use crate::{
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    semantic_index::build::token_in_special_context,
};

pub(super) fn collect_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> FileNameIndex {
    let tree = db.parse(HirFileId::from(file_id));
    let text = db.file_text(file_id);
    let mut occurrences: FxHashMap<SmolStr, Vec<NameOccurrence>> = FxHashMap::default();

    for event in tree.root().elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        if !token.kind().name_like() {
            continue;
        }
        let Some(range) = token.text_range() else {
            continue;
        };
        push_occurrence(
            &mut occurrences,
            &text,
            range,
            SyntaxTokenPtr::from_token(token),
            token_in_special_context(token),
        );
    }

    add_macro_argument_occurrences(db, file_id, &mut occurrences);

    FileNameIndex {
        occurrences: occurrences
            .into_iter()
            .map(|(name, mut entries)| {
                entries.sort_by_key(|occurrence| occurrence.range.start());
                entries.dedup_by(|lhs, rhs| lhs.range == rhs.range);
                (name, entries.into_boxed_slice())
            })
            .collect(),
    }
}

fn push_occurrence(
    occurrences: &mut FxHashMap<SmolStr, Vec<NameOccurrence>>,
    text: &str,
    range: utils::line_index::TextRange,
    ptr: SyntaxTokenPtr,
    special: bool,
) {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let Some(name) = text.get(start..end) else {
        return;
    };
    if name.is_empty() {
        return;
    }
    occurrences.entry(SmolStr::new(name)).or_default().push(NameOccurrence { range, ptr, special });
}

/// Macro arguments often live only in the preprocessor model, not as
/// name-like CST tokens. One model walk per file records them so find-refs
/// of the actual argument still hits the source token.
fn add_macro_argument_occurrences(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
    occurrences: &mut FxHashMap<SmolStr, Vec<NameOccurrence>>,
) {
    let preproc: &dyn PreprocDb = db;
    let mapped = preproc.source_preproc_model(file_id);
    let Ok(mapped) = mapped.as_ref().as_ref() else {
        return;
    };
    let text = db.file_text(file_id);
    for call in mapped.model.macro_calls().iter() {
        for argument in &call.arguments {
            for token in &argument.tokens {
                let Some(source_range) = token.range else {
                    continue;
                };
                let Ok(range) = mapped.source_map.map_range(source_range) else {
                    continue;
                };
                let Ok(token_file) = mapped.source_map.file_id(source_range.source) else {
                    continue;
                };
                if token_file != file_id || token.value.is_empty() {
                    continue;
                }
                if !token
                    .value
                    .chars()
                    .next()
                    .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
                {
                    continue;
                }
                push_occurrence(
                    occurrences,
                    &text,
                    range,
                    SyntaxTokenPtr::from_kind_range(TokenKind::IDENTIFIER, range),
                    false,
                );
            }
        }
    }
}
