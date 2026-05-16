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

impl CompletionItem {
    pub fn sort_text(&self) -> String {
        format!("{}:{}", self.kind.sort_rank(), self.label)
    }
}

impl CompletionItemKind {
    fn sort_rank(self) -> u8 {
        match self {
            CompletionItemKind::Text => 0,
            CompletionItemKind::Snippet => 1,
            CompletionItemKind::Keyword => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_text_orders_symbols_snippets_before_plain_keywords() {
        let text = item("foo", CompletionItemKind::Text);
        let snippet = item("foo", CompletionItemKind::Snippet);
        let keyword = item("foo", CompletionItemKind::Keyword);

        assert!(text.sort_text() < snippet.sort_text());
        assert!(snippet.sort_text() < keyword.sort_text());
    }

    fn item(label: &str, kind: CompletionItemKind) -> CompletionItem {
        CompletionItem { label: label.to_string(), kind, edit: None, snippet_edit: None }
    }
}
