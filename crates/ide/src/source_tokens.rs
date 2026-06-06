use hir::preproc::{TokenProvenance, macro_expansion_provenances_at};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, WalkEvent,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::db::root_db::RootDb;

#[derive(Debug, Clone)]
pub(crate) struct SourceTokenSelection<'tree> {
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

pub(crate) fn token_candidates_at_offset<'tree>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: impl Fn(TokenKind) -> usize,
) -> Option<SourceTokenSelection<'tree>> {
    match provenance_token_candidates_at_offset(db, file_id, root, offset) {
        ProvenanceTokenLookup::Available(selection) => return Some(selection),
        ProvenanceTokenLookup::Unavailable => return None,
        ProvenanceTokenLookup::NotApplicable => {}
    }

    let token = root.token_at_offset(offset).pick_bext_token(precedence)?;
    Some(SourceTokenSelection { range: token.text_range()?, tokens: vec![token] })
}

enum ProvenanceTokenLookup<'tree> {
    Available(SourceTokenSelection<'tree>),
    Unavailable,
    NotApplicable,
}

fn provenance_token_candidates_at_offset<'tree>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
) -> ProvenanceTokenLookup<'tree> {
    let Ok(provenances) = macro_expansion_provenances_at(db, file_id, offset) else {
        return ProvenanceTokenLookup::NotApplicable;
    };

    let mut source_ranges = Vec::new();
    for provenance in provenances {
        for token in provenance.tokens {
            let Some(range) = source_token_range_for_offset(&token.provenance, file_id, offset)
            else {
                continue;
            };
            if !source_ranges.contains(&range) {
                source_ranges.push(range);
            }
        }
    }

    if source_ranges.is_empty() {
        return ProvenanceTokenLookup::NotApplicable;
    }

    let tokens = tokens_with_exact_ranges(root, &source_ranges);
    if tokens.is_empty() {
        return ProvenanceTokenLookup::Unavailable;
    }

    let range = covering_range(&source_ranges);
    ProvenanceTokenLookup::Available(SourceTokenSelection { range, tokens })
}

fn source_token_range_for_offset(
    provenance: &TokenProvenance,
    file_id: FileId,
    offset: TextSize,
) -> Option<TextRange> {
    let (source, range) = match provenance {
        TokenProvenance::SourceToken { source, range }
        | TokenProvenance::MacroArgument { source, range, .. } => (source, *range),
        TokenProvenance::MacroBody { .. }
        | TokenProvenance::Predefine { .. }
        | TokenProvenance::Builtin { .. }
        | TokenProvenance::Unavailable(_) => return None,
    };
    (source.file_id() == file_id && range.contains_inclusive(offset)).then_some(range)
}

fn tokens_with_exact_ranges<'tree>(
    root: SyntaxNode<'tree>,
    ranges: &[TextRange],
) -> Vec<SyntaxTokenWithParent<'tree>> {
    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(range) = token.text_range() else {
            continue;
        };
        if ranges.contains(&range) && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    tokens
}

fn covering_range(ranges: &[TextRange]) -> TextRange {
    let start = ranges.iter().map(|range| range.start()).min().unwrap_or_default();
    let end = ranges.iter().map(|range| range.end()).max().unwrap_or_default();
    TextRange::new(start, end)
}
