use hir::{
    base_db::source_db::SourcePreprocQueryError,
    preproc::{
        EmittedTokenProvenance, MacroDefinitionId, MacroExpansionProvenance, MappedPreprocSource,
        PreprocError, TokenProvenance, macro_expansion_provenances_at,
    },
};
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, WalkEvent,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::db::root_db::RootDb;

#[derive(Debug, Clone)]
pub(crate) enum SourceTokenSelection<'tree> {
    NormalSyntax(NormalSyntaxSelection<'tree>),
    Preproc(PreprocTokenSelection<'tree>),
    Unavailable(PreprocTokenUnavailable),
    Ambiguous(PreprocTokenAmbiguity),
}

#[derive(Debug, Clone)]
pub(crate) struct NormalSyntaxSelection<'tree> {
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreprocTokenSelection<'tree> {
    pub range: TextRange,
    pub hits: Vec<PreprocTokenHit>,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenUnavailable {
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenAmbiguity {
    pub range: TextRange,
    pub hits: Vec<PreprocTokenHit>,
}

#[derive(Debug, Default)]
pub(crate) struct SourceTokenRequestCache {
    provenance_by_offset:
        FxHashMap<(FileId, TextSize), Result<Vec<MacroExpansionProvenance>, PreprocError>>,
}

impl SourceTokenRequestCache {
    fn macro_expansion_provenances_at(
        &mut self,
        db: &RootDb,
        file_id: FileId,
        offset: TextSize,
    ) -> Result<Vec<MacroExpansionProvenance>, PreprocError> {
        self.provenance_by_offset
            .entry((file_id, offset))
            .or_insert_with(|| macro_expansion_provenances_at(db, file_id, offset))
            .clone()
    }

    #[cfg(test)]
    fn macro_expansion_provenances_at_with(
        &mut self,
        file_id: FileId,
        offset: TextSize,
        compute: impl FnOnce() -> Result<Vec<MacroExpansionProvenance>, PreprocError>,
    ) -> Result<Vec<MacroExpansionProvenance>, PreprocError> {
        self.provenance_by_offset.entry((file_id, offset)).or_insert_with(compute).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenHit {
    pub expansion: usize,
    pub call: usize,
    pub emitted_token: usize,
    pub display_range: TextRange,
    pub source_range: TextRange,
    pub provenance: PreprocTokenProvenance,
    target: PreprocSemanticTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreprocTokenProvenance {
    SourceToken {
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroBody {
        call: usize,
        definition_id: MacroDefinitionId,
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroArgument {
        call: usize,
        argument_index: usize,
        source: MappedPreprocSource,
        range: TextRange,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreprocSemanticTarget {
    SourceToken { source: MappedPreprocSource, range: TextRange },
    MacroBody { definition_id: MacroDefinitionId, source: MappedPreprocSource, range: TextRange },
}

pub(crate) fn token_candidates_at_offset<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
) -> Option<SourceTokenSelection<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    let mut cache = SourceTokenRequestCache::default();
    token_candidates_at_offset_with_cache(db, file_id, root, offset, precedence, &mut cache)
}

pub(crate) fn token_candidates_at_offset_with_cache<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    cache: &mut SourceTokenRequestCache,
) -> Option<SourceTokenSelection<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    match provenance_token_candidates_at_offset(db, file_id, root, offset, &precedence, cache) {
        ProvenanceTokenLookup::Available(selection) => {
            return Some(SourceTokenSelection::Preproc(selection));
        }
        ProvenanceTokenLookup::Unavailable(unavailable) => {
            return Some(SourceTokenSelection::Unavailable(unavailable));
        }
        ProvenanceTokenLookup::Ambiguous(ambiguous) => {
            return Some(SourceTokenSelection::Ambiguous(ambiguous));
        }
        ProvenanceTokenLookup::NotApplicable => {}
    }

    normal_syntax_selection_at_offset(root, offset, &precedence)
        .map(SourceTokenSelection::NormalSyntax)
}

fn normal_syntax_selection_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> Option<NormalSyntaxSelection<'tree>> {
    let token = root.token_at_offset(offset).pick_bext_token(precedence)?;
    Some(NormalSyntaxSelection { range: token.text_range()?, tokens: vec![token] })
}

enum ProvenanceTokenLookup<'tree> {
    Available(PreprocTokenSelection<'tree>),
    Unavailable(PreprocTokenUnavailable),
    Ambiguous(PreprocTokenAmbiguity),
    NotApplicable,
}

fn provenance_token_candidates_at_offset<'tree>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    cache: &mut SourceTokenRequestCache,
) -> ProvenanceTokenLookup<'tree> {
    let provenances = match cache.macro_expansion_provenances_at(db, file_id, offset) {
        Ok(provenances) => provenances,
        Err(PreprocError::SourceQuery(SourcePreprocQueryError::UnsupportedFileKind(_))) => {
            return ProvenanceTokenLookup::NotApplicable;
        }
        Err(_) => {
            return ProvenanceTokenLookup::Unavailable(PreprocTokenUnavailable {
                range: TextRange::empty(offset),
            });
        }
    };
    if provenances.is_empty() {
        return ProvenanceTokenLookup::NotApplicable;
    }

    match preproc_hits_at_offset(&provenances, file_id, offset) {
        PreprocHitLookup::Available { range, hits } => {
            let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &hits)
            else {
                return ProvenanceTokenLookup::Unavailable(PreprocTokenUnavailable { range });
            };
            ProvenanceTokenLookup::Available(PreprocTokenSelection { range, hits, tokens })
        }
        PreprocHitLookup::Unavailable { range } => {
            ProvenanceTokenLookup::Unavailable(PreprocTokenUnavailable { range })
        }
        PreprocHitLookup::Ambiguous { range, hits } => {
            ProvenanceTokenLookup::Ambiguous(PreprocTokenAmbiguity { range, hits })
        }
    }
}

enum PreprocHitLookup {
    Available { range: TextRange, hits: Vec<PreprocTokenHit> },
    Unavailable { range: TextRange },
    Ambiguous { range: TextRange, hits: Vec<PreprocTokenHit> },
}

fn preproc_hits_at_offset(
    provenances: &[MacroExpansionProvenance],
    file_id: FileId,
    offset: TextSize,
) -> PreprocHitLookup {
    let mut hits = Vec::new();
    for expansion in provenances {
        for token in &expansion.tokens {
            let Some(hit) = preproc_hit_for_token(expansion, token, file_id, offset) else {
                continue;
            };
            push_unique_preproc_hit(&mut hits, hit);
        }
    }

    if hits.is_empty() {
        return PreprocHitLookup::Unavailable {
            range: covering_range(
                &provenances
                    .iter()
                    .map(|provenance| provenance.expansion.call.range)
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|| TextRange::empty(offset)),
        };
    }

    let range = covering_range(&hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
        .unwrap_or_else(|| TextRange::empty(offset));
    match hits.len() {
        0 => unreachable!(),
        1 => PreprocHitLookup::Available { range, hits },
        _ => PreprocHitLookup::Ambiguous { range, hits },
    }
}

fn preproc_hit_for_token(
    expansion: &MacroExpansionProvenance,
    token: &EmittedTokenProvenance,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocTokenHit> {
    let (source, range, provenance, target, call) = match &token.provenance {
        TokenProvenance::SourceToken { source, range } => (
            source.clone(),
            *range,
            PreprocTokenProvenance::SourceToken { source: source.clone(), range: *range },
            PreprocSemanticTarget::SourceToken { source: source.clone(), range: *range },
            expansion.expansion.call.id.raw(),
        ),
        TokenProvenance::MacroBody { call, definition_id, source, range } => (
            source.clone(),
            *range,
            PreprocTokenProvenance::MacroBody {
                call: call.id.raw(),
                definition_id: *definition_id,
                source: source.clone(),
                range: *range,
            },
            PreprocSemanticTarget::MacroBody {
                definition_id: *definition_id,
                source: source.clone(),
                range: *range,
            },
            call.id.raw(),
        ),
        TokenProvenance::MacroArgument { call, argument_index, source, range } => (
            source.clone(),
            *range,
            PreprocTokenProvenance::MacroArgument {
                call: call.id.raw(),
                argument_index: *argument_index,
                source: source.clone(),
                range: *range,
            },
            PreprocSemanticTarget::SourceToken { source: source.clone(), range: *range },
            call.id.raw(),
        ),
        TokenProvenance::Predefine { .. }
        | TokenProvenance::Builtin { .. }
        | TokenProvenance::Unavailable(_) => return None,
    };

    if source.file_id() != Some(file_id) || !range.contains(offset) {
        return None;
    }

    Some(PreprocTokenHit {
        expansion: expansion.expansion.id.raw(),
        call,
        emitted_token: token.token.raw(),
        display_range: token.display_range,
        source_range: range,
        provenance,
        target,
    })
}

fn push_unique_preproc_hit(hits: &mut Vec<PreprocTokenHit>, hit: PreprocTokenHit) {
    if hits.iter().any(|existing| existing.target == hit.target) {
        return;
    }
    hits.push(hit);
}

fn syntax_tokens_for_preproc_hit<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut tokens = Vec::new();
    let mut best_precedence = 0;
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(token_range) = token.text_range() else {
            continue;
        };
        if !token_range.contains(offset)
            || !hits.iter().any(|hit| hit.source_range.intersect(token_range).is_some())
        {
            continue;
        }

        let token_precedence = precedence(token.kind());
        if token_precedence > best_precedence {
            tokens.clear();
            best_precedence = token_precedence;
        }
        if token_precedence == best_precedence && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn covering_range(ranges: &[TextRange]) -> Option<TextRange> {
    let start = ranges.iter().map(|range| range.start()).min()?;
    let end = ranges.iter().map(|range| range.end()).max()?;
    Some(TextRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use syntax::{SyntaxTree, token::TokenKindExt};

    use super::*;

    #[test]
    fn source_tokens_provenance_source_range_hit_test_is_half_open() {
        let file_id = FileId(0);
        let range = TextRange::new(5.into(), 10.into());
        let provenance = TokenProvenance::SourceToken {
            source: MappedPreprocSource::RealFile { file_id },
            range,
        };

        assert!(
            preproc_hit_for_raw_provenance(&provenance, file_id, 5.into()).is_some(),
            "range start should hit"
        );
        assert!(
            preproc_hit_for_raw_provenance(&provenance, file_id, 9.into()).is_some(),
            "offset before range end should hit"
        );
        assert!(
            preproc_hit_for_raw_provenance(&provenance, file_id, 10.into()).is_none(),
            "range end should not hit"
        );
    }

    #[test]
    fn source_tokens_preproc_range_mismatch_still_selects_by_identity() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
        let file_id = FileId(0);
        let provenance_range = TextRange::new(
            parser_range.start() + TextSize::from(1),
            parser_range.end() - TextSize::from(1),
        );
        let hit = test_source_hit(file_id, provenance_range, 0);

        let ProvenanceTokenLookup::Available(selection) = preproc_selection_from_hits(
            root,
            offset,
            &test_precedence,
            vec![hit],
            provenance_range,
        ) else {
            panic!("preproc identity hit should select without exact parser range equality");
        };

        assert_eq!(selection.range, provenance_range);
        assert_eq!(selection.hits.len(), 1);
        assert_eq!(selection.tokens.len(), 1);
        assert_eq!(selection.tokens[0].text_range(), Some(parser_range));
        assert_ne!(selection.tokens[0].text_range(), Some(provenance_range));
    }

    #[test]
    fn source_tokens_preproc_owned_unresolved_does_not_use_normal_syntax_fallback() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        assert!(
            normal_syntax_selection_at_offset(root, offset, &test_precedence).is_some(),
            "test setup must have an ordinary syntax token that fallback could have selected"
        );

        let lookup =
            preproc_selection_from_hits(root, offset, &test_precedence, Vec::new(), parser_range);
        assert!(matches!(lookup, ProvenanceTokenLookup::Unavailable(_)));
    }

    #[test]
    fn source_tokens_normal_syntax_path_still_selects_non_preproc_offsets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        let selection = normal_syntax_selection_at_offset(root, offset, &test_precedence)
            .expect("normal syntax token expected");

        assert_eq!(selection.range, parser_range);
        assert_eq!(selection.tokens.len(), 1);
    }

    #[test]
    fn source_tokens_dedups_preproc_hits_for_same_semantic_target() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        let file_id = FileId(0);
        let hits = vec![
            test_source_hit(file_id, parser_range, 0),
            test_source_hit(file_id, parser_range, 1),
        ];

        let ProvenanceTokenLookup::Available(selection) =
            preproc_selection_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("same semantic target should dedup to one available preproc hit");
        };

        assert_eq!(selection.hits.len(), 1);
    }

    #[test]
    fn source_tokens_reports_ambiguous_preproc_hits_for_conflicting_targets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
        let file_id = FileId(0);
        let first = TextRange::new(parser_range.start(), parser_range.start() + TextSize::from(4));
        let second = TextRange::new(parser_range.start() + TextSize::from(1), parser_range.end());
        let hits = vec![test_source_hit(file_id, first, 0), test_source_hit(file_id, second, 1)];

        let ProvenanceTokenLookup::Ambiguous(ambiguous) =
            preproc_selection_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("conflicting preproc targets should be ambiguous");
        };

        assert_eq!(ambiguous.hits.len(), 2);
    }

    #[test]
    fn source_token_request_cache_reuses_provenance_lookup_for_repeated_reference_hits() {
        let mut cache = SourceTokenRequestCache::default();
        let mut lookups = 0usize;
        let file_id = FileId(0);
        let offset = TextSize::from(12);

        for _ in 0..3 {
            let result = cache
                .macro_expansion_provenances_at_with(file_id, offset, || {
                    lookups += 1;
                    Ok(Vec::new())
                })
                .unwrap();
            assert!(result.is_empty());
        }

        assert_eq!(lookups, 1, "repeated text hits at one offset should reuse the request cache");

        let _ = cache
            .macro_expansion_provenances_at_with(file_id, offset + TextSize::from(1), || {
                lookups += 1;
                Ok(Vec::new())
            })
            .unwrap();
        assert_eq!(lookups, 2, "different offsets should remain distinct cache entries");
    }

    fn root_and_offset<'tree>(
        text: &str,
        needle: &str,
        delta: u32,
    ) -> (SyntaxNode<'tree>, TextSize, TextRange) {
        let tree = Box::leak(Box::new(SyntaxTree::from_text(text, "test", "test.sv")));
        let root = tree.root().expect("test source should parse");
        let start = text.find(needle).expect("needle should exist");
        let range = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + needle.len()) as u32),
        );
        (root, range.start() + TextSize::from(delta), range)
    }

    fn test_source_hit(file_id: FileId, range: TextRange, emitted_token: usize) -> PreprocTokenHit {
        let source = MappedPreprocSource::RealFile { file_id };
        PreprocTokenHit {
            expansion: 0,
            call: 0,
            emitted_token,
            display_range: range,
            source_range: range,
            provenance: PreprocTokenProvenance::SourceToken { source: source.clone(), range },
            target: PreprocSemanticTarget::SourceToken { source, range },
        }
    }

    fn preproc_selection_from_hits<'tree>(
        root: SyntaxNode<'tree>,
        offset: TextSize,
        precedence: &impl Fn(TokenKind) -> usize,
        hits: Vec<PreprocTokenHit>,
        fallback_range: TextRange,
    ) -> ProvenanceTokenLookup<'tree> {
        let mut unique_hits = Vec::new();
        for hit in hits {
            if hit.source_range.contains(offset) {
                push_unique_preproc_hit(&mut unique_hits, hit);
            }
        }
        if unique_hits.is_empty() {
            return ProvenanceTokenLookup::Unavailable(PreprocTokenUnavailable {
                range: fallback_range,
            });
        }
        let range =
            covering_range(&unique_hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
                .unwrap_or(fallback_range);
        if unique_hits.len() > 1 {
            return ProvenanceTokenLookup::Ambiguous(PreprocTokenAmbiguity {
                range,
                hits: unique_hits,
            });
        }
        let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &unique_hits)
        else {
            return ProvenanceTokenLookup::Unavailable(PreprocTokenUnavailable { range });
        };
        ProvenanceTokenLookup::Available(PreprocTokenSelection { range, hits: unique_hits, tokens })
    }

    fn preproc_hit_for_raw_provenance(
        provenance: &TokenProvenance,
        file_id: FileId,
        offset: TextSize,
    ) -> Option<TextRange> {
        let (source, range) = match provenance {
            TokenProvenance::SourceToken { source, range } => (source, *range),
            _ => return None,
        };
        (source.file_id() == Some(file_id) && range.contains(offset)).then_some(range)
    }

    fn test_precedence(kind: TokenKind) -> usize {
        usize::from(kind.name_like())
    }
}
