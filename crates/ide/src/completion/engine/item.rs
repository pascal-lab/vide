use std::fmt;

use utils::text_edit::TextEditItem;

#[derive(Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub edit: Option<TextEditItem>,
    pub snippet_edit: Option<TextEditItem>,
    sort_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionItemKind {
    Text,
    Keyword,
    Snippet,
}

impl CompletionItem {
    pub(super) fn new(
        label: String,
        kind: CompletionItemKind,
        edit: Option<TextEditItem>,
        snippet_edit: Option<TextEditItem>,
        sort_text: String,
    ) -> Self {
        Self { label, kind, edit, snippet_edit, sort_text }
    }

    pub fn sort_text(&self) -> String {
        self.sort_text.clone()
    }
}

impl fmt::Debug for CompletionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionItem")
            .field("label", &self.label)
            .field("kind", &self.kind)
            .field("edit", &self.edit)
            .field("snippet_edit", &self.snippet_edit)
            .finish()
    }
}
