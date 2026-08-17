use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use syntax::{SyntaxElement, WalkEvent, has_text_range::HasTextRange, token::TokenKindExt};
use vfs::FileId;

use super::{FileNameIndex, NameOccurrence};
use crate::{
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    semantic_target::preproc::syntax_token_emitted_token_id,
};

pub(super) fn collect_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> FileNameIndex {
    // Use the compilation parse so `ifdef` / predefines match the file the
    // user sees. Include expansion is still the cost of that parse; the parse
    // LRU, not this table, decides whether the tree stays resident.
    let tree = db.parse(HirFileId::from(file_id));
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
        let name = token.tok.value_text();
        if name.is_empty() {
            continue;
        }
        occurrences.entry(SmolStr::new(name)).or_default().push(NameOccurrence {
            range,
            kind: token.kind(),
            emitted: syntax_token_emitted_token_id(&token),
        });
    }

    FileNameIndex {
        occurrences: occurrences
            .into_iter()
            .map(|(name, mut entries)| {
                entries.sort_by_key(|occurrence| {
                    (occurrence.range.start(), occurrence.emitted.map(|id| id.raw()))
                });
                entries.dedup_by(|lhs, rhs| {
                    lhs.range == rhs.range && lhs.kind == rhs.kind && lhs.emitted == rhs.emitted
                });
                (name, entries.into_boxed_slice())
            })
            .collect(),
    }
}
