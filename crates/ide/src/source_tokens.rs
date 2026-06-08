use hir::{
    base_db::source_db::{SourceDb, SourcePreprocQueryError},
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
pub(crate) enum SourceTokenResolution<'tree> {
    Resolved(SourceTokenSelection<'tree>),
    Blocked(SourceTokenBlock),
}

impl<'tree> SourceTokenResolution<'tree> {
    pub(crate) fn resolved(self) -> Option<SourceTokenSelection<'tree>> {
        match self {
            Self::Resolved(selection) => Some(selection),
            Self::Blocked(SourceTokenBlock { .. }) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTokenSelection<'tree> {
    pub origin: SourceTokenOrigin,
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SourceTokenSelection<'tree> {
    fn normal_syntax(range: TextRange, tokens: Vec<SyntaxTokenWithParent<'tree>>) -> Self {
        Self { origin: SourceTokenOrigin::NormalSyntax, range, tokens }
    }

    fn preproc(
        range: TextRange,
        hits: Vec<PreprocTokenHit>,
        tokens: Vec<SyntaxTokenWithParent<'tree>>,
    ) -> Self {
        Self { origin: SourceTokenOrigin::Preproc { hits }, range, tokens }
    }

    pub(crate) fn into_parts(self) -> (TextRange, Vec<SyntaxTokenWithParent<'tree>>) {
        let Self { origin, range, tokens } = self;
        match origin {
            SourceTokenOrigin::NormalSyntax => (range, tokens),
            SourceTokenOrigin::Preproc { hits } => {
                let _hit_count = hits.len();
                (range, tokens)
            }
        }
    }

    pub(crate) fn into_tokens(self) -> Vec<SyntaxTokenWithParent<'tree>> {
        self.into_parts().1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTokenOrigin {
    NormalSyntax,
    Preproc { hits: Vec<PreprocTokenHit> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTokenDomain {
    Preproc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceTokenBlock {
    pub domain: SourceTokenDomain,
    pub range: TextRange,
    pub reason: SourceTokenBlockReason,
}

impl SourceTokenBlock {
    fn preproc_unavailable(range: TextRange) -> Self {
        Self {
            domain: SourceTokenDomain::Preproc,
            range,
            reason: SourceTokenBlockReason::Unavailable,
        }
    }

    fn preproc_ambiguous(range: TextRange, hits: Vec<PreprocTokenHit>) -> Self {
        Self {
            domain: SourceTokenDomain::Preproc,
            range,
            reason: SourceTokenBlockReason::Ambiguous { hits },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTokenBlockReason {
    Unavailable,
    Ambiguous { hits: Vec<PreprocTokenHit> },
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

pub(crate) fn source_token_resolution_at_offset<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
) -> Option<SourceTokenResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    let mut cache = SourceTokenRequestCache::default();
    source_token_resolution_at_offset_with_cache(db, file_id, root, offset, precedence, &mut cache)
}

pub(crate) fn source_token_resolution_at_offset_with_cache<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    cache: &mut SourceTokenRequestCache,
) -> Option<SourceTokenResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    match preproc_source_token_at_offset(db, file_id, root, offset, &precedence, cache) {
        SourceTokenProviderResult::NotApplicable => {
            normal_syntax_source_token_at_offset(root, offset, &precedence).into_resolution()
        }
        result => result.into_resolution(),
    }
}

fn normal_syntax_source_token_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> SourceTokenProviderResult<'tree> {
    let Some(token) = root.token_at_offset(offset).pick_bext_token(precedence) else {
        return SourceTokenProviderResult::NotApplicable;
    };
    let Some(range) = token.text_range() else {
        return SourceTokenProviderResult::NotApplicable;
    };
    SourceTokenProviderResult::Resolved(SourceTokenSelection::normal_syntax(range, vec![token]))
}

enum SourceTokenProviderResult<'tree> {
    Resolved(SourceTokenSelection<'tree>),
    Blocked(SourceTokenBlock),
    NotApplicable,
}

impl<'tree> SourceTokenProviderResult<'tree> {
    fn into_resolution(self) -> Option<SourceTokenResolution<'tree>> {
        match self {
            Self::Resolved(selection) => Some(SourceTokenResolution::Resolved(selection)),
            Self::Blocked(block) => Some(SourceTokenResolution::Blocked(block)),
            Self::NotApplicable => None,
        }
    }
}

fn preproc_source_token_at_offset<'tree>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    cache: &mut SourceTokenRequestCache,
) -> SourceTokenProviderResult<'tree> {
    if !source_macro_invocation_may_cover_offset(db.file_text(file_id).as_ref(), offset) {
        return SourceTokenProviderResult::NotApplicable;
    }

    let provenances = match cache.macro_expansion_provenances_at(db, file_id, offset) {
        Ok(provenances) => provenances,
        Err(PreprocError::SourceQuery(SourcePreprocQueryError::UnsupportedFileKind(_))) => {
            return SourceTokenProviderResult::NotApplicable;
        }
        Err(_) => {
            return SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_unavailable(
                TextRange::empty(offset),
            ));
        }
    };
    if provenances.is_empty() {
        return SourceTokenProviderResult::NotApplicable;
    }

    match preproc_hits_at_offset(&provenances, file_id, offset) {
        PreprocHitLookup::Available { range, hits } => {
            let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &hits)
            else {
                return SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_unavailable(
                    range,
                ));
            };
            SourceTokenProviderResult::Resolved(SourceTokenSelection::preproc(range, hits, tokens))
        }
        PreprocHitLookup::Unavailable { range } => {
            SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_unavailable(range))
        }
        PreprocHitLookup::Ambiguous { range, hits } => {
            SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_ambiguous(range, hits))
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
        | TokenProvenance::TokenPaste { .. }
        | TokenProvenance::Stringification { .. }
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

fn source_macro_invocation_may_cover_offset(text: &str, offset: TextSize) -> bool {
    let offset = usize::from(offset);
    if offset > text.len() || !text.is_char_boundary(offset) {
        return false;
    }

    let search_end = text[offset..].chars().next().map_or(offset, |ch| offset + ch.len_utf8());
    let prefix = &text[..search_end];
    for (tick, _) in prefix.match_indices('`').rev() {
        match macro_invocation_candidate_end(text, tick) {
            MacroInvocationCandidate::RangeEnd(end) if offset <= end => return true,
            MacroInvocationCandidate::RangeEnd(_) => {}
            MacroInvocationCandidate::Malformed => return true,
            MacroInvocationCandidate::NotMacro => {}
        }
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroInvocationCandidate {
    RangeEnd(usize),
    Malformed,
    NotMacro,
}

fn macro_invocation_candidate_end(text: &str, tick: usize) -> MacroInvocationCandidate {
    let Some(after_tick) = text.get(tick + 1..) else {
        return MacroInvocationCandidate::Malformed;
    };
    let Some((name_start_offset, first)) = after_tick.char_indices().next() else {
        return MacroInvocationCandidate::Malformed;
    };
    let name_start = tick + 1 + name_start_offset;
    let name_end = if first == '\\' {
        let Some((end, _)) = text[name_start..].char_indices().find(|(_, ch)| ch.is_whitespace())
        else {
            return MacroInvocationCandidate::Malformed;
        };
        name_start + end
    } else if is_macro_ident_start(first) {
        text[name_start..]
            .char_indices()
            .find_map(|(index, ch)| (!is_macro_ident_continue(ch)).then_some(name_start + index))
            .unwrap_or(text.len())
    } else {
        return MacroInvocationCandidate::NotMacro;
    };

    let after_name = &text[name_end..];
    let Some((next_offset, next)) = after_name.char_indices().find(|(_, ch)| !ch.is_whitespace())
    else {
        return MacroInvocationCandidate::RangeEnd(name_end);
    };
    if next != '(' {
        return MacroInvocationCandidate::RangeEnd(name_end);
    }
    let open = name_end + next_offset;
    match balanced_paren_end(text, open) {
        Some(end) => MacroInvocationCandidate::RangeEnd(end),
        None => MacroInvocationCandidate::Malformed,
    }
}

fn balanced_paren_end(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut chars = text[open..].char_indices();
    while let Some((relative, ch)) = chars.next() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + relative + ch.len_utf8());
                }
            }
            '"' => {
                while let Some((_, string_ch)) = chars.next() {
                    if string_ch == '\\' {
                        let _ = chars.next();
                    } else if string_ch == '"' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn is_macro_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_macro_ident_continue(ch: char) -> bool {
    is_macro_ident_start(ch) || ch.is_ascii_digit() || ch == '$'
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

        let SourceTokenProviderResult::Resolved(selection) = preproc_provider_result_from_hits(
            root,
            offset,
            &test_precedence,
            vec![hit],
            provenance_range,
        ) else {
            panic!("preproc identity hit should select without exact parser range equality");
        };

        assert_eq!(selection.range, provenance_range);
        let SourceTokenOrigin::Preproc { hits } = &selection.origin else {
            panic!("preproc provider should produce a preproc-origin selection");
        };
        assert_eq!(hits.len(), 1);
        assert_eq!(selection.tokens.len(), 1);
        assert_eq!(selection.tokens[0].text_range(), Some(parser_range));
        assert_ne!(selection.tokens[0].text_range(), Some(provenance_range));
    }

    #[test]
    fn source_tokens_preproc_owned_unresolved_does_not_use_normal_syntax_fallback() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        assert!(
            matches!(
                normal_syntax_source_token_at_offset(root, offset, &test_precedence),
                SourceTokenProviderResult::Resolved(_)
            ),
            "test setup must have an ordinary syntax token that fallback could have selected"
        );

        let lookup = preproc_provider_result_from_hits(
            root,
            offset,
            &test_precedence,
            Vec::new(),
            parser_range,
        );
        assert!(matches!(
            lookup,
            SourceTokenProviderResult::Blocked(SourceTokenBlock {
                domain: SourceTokenDomain::Preproc,
                reason: SourceTokenBlockReason::Unavailable,
                ..
            })
        ));
    }

    #[test]
    fn source_tokens_normal_syntax_path_still_selects_non_preproc_offsets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        let SourceTokenProviderResult::Resolved(selection) =
            normal_syntax_source_token_at_offset(root, offset, &test_precedence)
        else {
            panic!("normal syntax token expected");
        };

        assert!(matches!(selection.origin, SourceTokenOrigin::NormalSyntax));
        assert_eq!(selection.range, parser_range);
        assert_eq!(selection.tokens.len(), 1);
    }

    #[test]
    fn source_tokens_macro_provenance_gate_skips_plain_identifiers() {
        let text = "module m; wire payload_i; endmodule\n";

        assert!(!source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
    }

    #[test]
    fn source_tokens_macro_provenance_gate_keeps_macro_names_and_arguments() {
        let text = "module m; wire `MAKE_DECL(payload_i); endmodule\n";

        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "`MAKE_DECL")));
        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
    }

    #[test]
    fn source_tokens_macro_provenance_gate_keeps_outer_arguments_after_nested_macros() {
        let text = "assign y = `OUTER(a, `INNER(b), payload_i);\n";

        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
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

        let SourceTokenProviderResult::Resolved(selection) =
            preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("same semantic target should dedup to one available preproc hit");
        };

        let SourceTokenOrigin::Preproc { hits } = selection.origin else {
            panic!("preproc provider should produce a preproc-origin selection");
        };
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn source_tokens_reports_ambiguous_preproc_hits_for_conflicting_targets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
        let file_id = FileId(0);
        let first = TextRange::new(parser_range.start(), parser_range.start() + TextSize::from(4));
        let second = TextRange::new(parser_range.start() + TextSize::from(1), parser_range.end());
        let hits = vec![test_source_hit(file_id, first, 0), test_source_hit(file_id, second, 1)];

        let SourceTokenProviderResult::Blocked(SourceTokenBlock {
            reason: SourceTokenBlockReason::Ambiguous { hits },
            ..
        }) = preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("conflicting preproc targets should be ambiguous");
        };

        assert_eq!(hits.len(), 2);
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

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).expect("needle should exist")).unwrap())
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

    fn preproc_provider_result_from_hits<'tree>(
        root: SyntaxNode<'tree>,
        offset: TextSize,
        precedence: &impl Fn(TokenKind) -> usize,
        hits: Vec<PreprocTokenHit>,
        fallback_range: TextRange,
    ) -> SourceTokenProviderResult<'tree> {
        let mut unique_hits = Vec::new();
        for hit in hits {
            if hit.source_range.contains(offset) {
                push_unique_preproc_hit(&mut unique_hits, hit);
            }
        }
        if unique_hits.is_empty() {
            return SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_unavailable(
                fallback_range,
            ));
        }
        let range =
            covering_range(&unique_hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
                .unwrap_or(fallback_range);
        if unique_hits.len() > 1 {
            return SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_ambiguous(
                range,
                unique_hits,
            ));
        }
        let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &unique_hits)
        else {
            return SourceTokenProviderResult::Blocked(SourceTokenBlock::preproc_unavailable(
                range,
            ));
        };
        SourceTokenProviderResult::Resolved(SourceTokenSelection::preproc(
            range,
            unique_hits,
            tokens,
        ))
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
