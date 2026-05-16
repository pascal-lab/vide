use utils::text_edit::TextEditItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub edit: Option<TextEditItem>,
    pub snippet_edit: Option<TextEditItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text,
    Keyword,
    Snippet,
}
