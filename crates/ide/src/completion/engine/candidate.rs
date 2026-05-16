use rustc_hash::FxHashSet;
use utils::text_edit::{TextEditItem, TextRange};

use super::{CompletionItem, CompletionItemKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CandidateIdentity {
    label: String,
    kind: CompletionItemKind,
    edit: Option<TextEditItem>,
    snippet_edit: Option<TextEditItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompletionCandidate {
    label: String,
    kind: CompletionItemKind,
    edit: Option<TextEditItem>,
    snippet_edit: Option<TextEditItem>,
    rank: CandidateRank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CandidateRank {
    SemanticText,
    SemanticSnippet,
    SyntaxSnippet,
    SyntaxKeyword,
}

impl CandidateRank {
    fn as_u8(self) -> u8 {
        match self {
            CandidateRank::SemanticText => 0,
            CandidateRank::SemanticSnippet => 1,
            CandidateRank::SyntaxSnippet => 2,
            CandidateRank::SyntaxKeyword => 3,
        }
    }
}

impl CompletionCandidate {
    pub(super) fn text(label: impl Into<String>, replacement: TextRange) -> Self {
        let label = label.into();
        Self::text_edit(label.clone(), replacement, label)
    }

    pub(super) fn text_edit(
        label: impl Into<String>,
        replacement: TextRange,
        text: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(replacement, text.into())),
            snippet_edit: None,
            rank: CandidateRank::SemanticText,
        }
    }

    pub(super) fn keyword(label: impl Into<String>, replacement: TextRange) -> Self {
        let label = label.into();
        Self {
            label: label.clone(),
            kind: CompletionItemKind::Keyword,
            edit: Some(TextEditItem::replace(replacement, label)),
            snippet_edit: None,
            rank: CandidateRank::SyntaxKeyword,
        }
    }

    pub(super) fn text_snippet(
        label: impl Into<String>,
        replacement: TextRange,
        plain: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(replacement, plain.into())),
            snippet_edit: Some(TextEditItem::replace(replacement, snippet.into())),
            rank: CandidateRank::SemanticText,
        }
    }

    pub(super) fn semantic_snippet(
        label: impl Into<String>,
        replacement: TextRange,
        plain: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Snippet,
            edit: Some(TextEditItem::replace(replacement, plain.into())),
            snippet_edit: Some(TextEditItem::replace(replacement, snippet.into())),
            rank: CandidateRank::SemanticSnippet,
        }
    }

    pub(super) fn snippet(
        label: impl Into<String>,
        replacement: TextRange,
        plain: impl Into<String>,
        snippet: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            kind: CompletionItemKind::Snippet,
            edit: Some(TextEditItem::replace(replacement, plain.into())),
            snippet_edit: Some(TextEditItem::replace(replacement, snippet.into())),
            rank: CandidateRank::SyntaxSnippet,
        }
    }

    pub(super) fn label(&self) -> &str {
        &self.label
    }

    fn sort_key(&self) -> (u8, &str) {
        (self.rank.as_u8(), self.label())
    }

    fn identity(&self) -> CandidateIdentity {
        CandidateIdentity {
            label: self.label.clone(),
            kind: self.kind,
            edit: self.edit.clone(),
            snippet_edit: self.snippet_edit.clone(),
        }
    }

    fn into_item(self) -> CompletionItem {
        let sort_text = format!("{:02}:{}", self.rank.as_u8(), self.label);
        CompletionItem::new(self.label, self.kind, self.edit, self.snippet_edit, sort_text)
    }
}

pub(super) fn finalize_candidates(
    candidates: impl IntoIterator<Item = CompletionCandidate>,
    prefix: &str,
) -> Vec<CompletionItem> {
    let mut candidates = candidates
        .into_iter()
        .filter(|candidate| candidate.label().starts_with(prefix))
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));

    let mut seen = FxHashSet::default();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.identity()))
        .map(CompletionCandidate::into_item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finalization_filters_deduplicates_and_sorts_candidates() {
        let range = TextRange::empty(0.into());
        let mut semantic_while =
            CompletionCandidate::snippet("while", range, "while () begin end", "while (${1})");
        semantic_while.rank = CandidateRank::SemanticSnippet;
        let items = finalize_candidates(
            [
                CompletionCandidate::keyword("wire", range),
                CompletionCandidate::text("word", range),
                CompletionCandidate::text("word", range),
                CompletionCandidate::snippet("while", range, "while () begin end", "while (${1})"),
                semantic_while,
                CompletionCandidate::keyword("assign", range),
            ],
            "w",
        );

        let labels = items.iter().map(|item| item.label.as_str()).collect::<Vec<_>>();
        let kinds = items.iter().map(|item| item.kind).collect::<Vec<_>>();
        let sort_texts = items.iter().map(|item| item.sort_text()).collect::<Vec<_>>();

        assert_eq!(labels, ["word", "while", "wire"]);
        assert_eq!(
            kinds,
            [CompletionItemKind::Text, CompletionItemKind::Snippet, CompletionItemKind::Keyword]
        );
        assert_eq!(sort_texts, ["00:word", "01:while", "03:wire"]);
    }
}
