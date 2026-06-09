use hir::{
    base_db::source_db::{SourceDb, SourcePreprocQueryError},
    preproc::{
        EmittedTokenProvenance, MacroArgumentTokenIdentity, MacroBodyTokenIdentity,
        MacroDefinitionId, MacroExpansionProvenance, MacroTokenIdentity, MappedPreprocSource,
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
pub(crate) enum SourceTargetResolution<'tree> {
    Resolved(SourceTarget<'tree>),
    Blocked(SourceTargetBlock),
}

impl<'tree> SourceTargetResolution<'tree> {
    pub(crate) fn resolved(self) -> Option<SourceTarget<'tree>> {
        match self {
            Self::Resolved(selection) => Some(selection),
            Self::Blocked(SourceTargetBlock { .. }) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTarget<'tree> {
    pub origin: SourceTargetOrigin,
    pub range: TextRange,
    pub tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SourceTarget<'tree> {
    fn normal_syntax(range: TextRange, tokens: Vec<SyntaxTokenWithParent<'tree>>) -> Self {
        Self { origin: SourceTargetOrigin::NormalSyntax, range, tokens }
    }

    fn preproc(
        range: TextRange,
        hits: Vec<PreprocTokenHit>,
        tokens: Vec<SyntaxTokenWithParent<'tree>>,
    ) -> Self {
        Self { origin: SourceTargetOrigin::Preproc { hits }, range, tokens }
    }

    pub(crate) fn into_parts(self) -> (TextRange, Vec<SyntaxTokenWithParent<'tree>>) {
        let Self { origin, range, tokens } = self;
        match origin {
            SourceTargetOrigin::NormalSyntax => (range, tokens),
            SourceTargetOrigin::Preproc { hits } => {
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
pub(crate) enum SourceTargetOrigin {
    NormalSyntax,
    Preproc { hits: Vec<PreprocTokenHit> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceTargetDomain {
    Preproc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceTargetBlock {
    pub domain: SourceTargetDomain,
    pub range: TextRange,
    pub reason: SourceTargetBlockReason,
}

impl SourceTargetBlock {
    fn preproc_unavailable(range: TextRange) -> Self {
        Self {
            domain: SourceTargetDomain::Preproc,
            range,
            reason: SourceTargetBlockReason::Unavailable,
        }
    }

    fn preproc_ambiguous(range: TextRange, hits: Vec<PreprocTokenHit>) -> Self {
        Self {
            domain: SourceTargetDomain::Preproc,
            range,
            reason: SourceTargetBlockReason::Ambiguous { hits },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceTargetBlockReason {
    Unavailable,
    Ambiguous { hits: Vec<PreprocTokenHit> },
}

#[derive(Debug, Default)]
pub(crate) struct SourceTargetRequestCache {
    provenance_by_offset:
        FxHashMap<(FileId, TextSize), Result<Vec<MacroExpansionProvenance>, PreprocError>>,
}

impl SourceTargetRequestCache {
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
    target: PreprocSourceTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreprocTokenProvenance {
    SourceToken {
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroBody {
        identity: MacroBodyTokenIdentity,
        call: usize,
        definition_id: MacroDefinitionId,
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroArgument {
        identity: MacroArgumentTokenIdentity,
        call: usize,
        argument_index: usize,
        source: MappedPreprocSource,
        range: TextRange,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreprocSourceTarget {
    SourceToken { source: MappedPreprocSource, range: TextRange },
    MacroBody { definition_id: MacroDefinitionId, source: MappedPreprocSource, range: TextRange },
}

pub(crate) fn source_target_at_offset<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
) -> Option<SourceTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    let mut cache = SourceTargetRequestCache::default();
    source_target_at_offset_with_cache(db, file_id, root, offset, precedence, &mut cache)
}

pub(crate) fn source_target_at_offset_with_cache<'tree, F>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: F,
    cache: &mut SourceTargetRequestCache,
) -> Option<SourceTargetResolution<'tree>>
where
    F: Fn(TokenKind) -> usize,
{
    match preproc_source_target_at_offset(db, file_id, root, offset, &precedence, cache) {
        SourceTargetProviderResult::NotApplicable => {
            normal_syntax_source_target_at_offset(root, offset, &precedence).into_resolution()
        }
        result => result.into_resolution(),
    }
}

fn normal_syntax_source_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> SourceTargetProviderResult<'tree> {
    let Some(token) = root.token_at_offset(offset).pick_bext_token(precedence) else {
        return SourceTargetProviderResult::NotApplicable;
    };
    let Some(range) = token.text_range() else {
        return SourceTargetProviderResult::NotApplicable;
    };
    SourceTargetProviderResult::Resolved(SourceTarget::normal_syntax(range, vec![token]))
}

enum SourceTargetProviderResult<'tree> {
    Resolved(SourceTarget<'tree>),
    Blocked(SourceTargetBlock),
    NotApplicable,
}

impl<'tree> SourceTargetProviderResult<'tree> {
    fn into_resolution(self) -> Option<SourceTargetResolution<'tree>> {
        match self {
            Self::Resolved(selection) => Some(SourceTargetResolution::Resolved(selection)),
            Self::Blocked(block) => Some(SourceTargetResolution::Blocked(block)),
            Self::NotApplicable => None,
        }
    }
}

fn preproc_source_target_at_offset<'tree>(
    db: &RootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    cache: &mut SourceTargetRequestCache,
) -> SourceTargetProviderResult<'tree> {
    if !source_macro_invocation_may_cover_offset(db.file_text(file_id).as_ref(), offset) {
        return SourceTargetProviderResult::NotApplicable;
    }

    let provenances = match cache.macro_expansion_provenances_at(db, file_id, offset) {
        Ok(provenances) => provenances,
        Err(PreprocError::SourceQuery(SourcePreprocQueryError::UnsupportedFileKind(_))) => {
            return SourceTargetProviderResult::NotApplicable;
        }
        Err(_) => {
            return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(
                TextRange::empty(offset),
            ));
        }
    };
    if provenances.is_empty() {
        return SourceTargetProviderResult::NotApplicable;
    }

    match preproc_hits_at_offset(&provenances, file_id, offset) {
        PreprocHitLookup::Available { range, hits } => {
            let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &hits)
            else {
                return SourceTargetProviderResult::Blocked(
                    SourceTargetBlock::preproc_unavailable(range),
                );
            };
            SourceTargetProviderResult::Resolved(SourceTarget::preproc(range, hits, tokens))
        }
        PreprocHitLookup::Unavailable { range } => {
            SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(range))
        }
        PreprocHitLookup::Ambiguous { range, hits } => {
            SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_ambiguous(range, hits))
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
            PreprocSourceTarget::SourceToken { source: source.clone(), range: *range },
            expansion.expansion.call.id.raw(),
        ),
        TokenProvenance::MacroBody { identity, call, definition_id, source, range } => (
            source.clone(),
            *range,
            PreprocTokenProvenance::MacroBody {
                identity: *identity,
                call: call.id.raw(),
                definition_id: *definition_id,
                source: source.clone(),
                range: *range,
            },
            PreprocSourceTarget::MacroBody {
                definition_id: *definition_id,
                source: source.clone(),
                range: *range,
            },
            call.id.raw(),
        ),
        TokenProvenance::MacroArgument { identity, call, argument_index, source, range } => (
            source.clone(),
            *range,
            PreprocTokenProvenance::MacroArgument {
                identity: *identity,
                call: call.id.raw(),
                argument_index: *argument_index,
                source: source.clone(),
                range: *range,
            },
            PreprocSourceTarget::SourceToken { source: source.clone(), range: *range },
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
    let identities = hits.iter().filter_map(macro_token_identity_for_hit).collect::<Vec<_>>();
    if !identities.is_empty() {
        return syntax_tokens_for_macro_identities(root, &identities);
    }

    return normal_syntax_source_target_at_offset(root, offset, precedence)
        .into_resolution()
        .and_then(SourceTargetResolution::resolved)
        .map(SourceTarget::into_tokens);
}

fn macro_token_identity_for_hit(hit: &PreprocTokenHit) -> Option<MacroTokenIdentity> {
    match hit.provenance {
        PreprocTokenProvenance::MacroBody { identity, .. } => {
            Some(MacroTokenIdentity::Body(identity))
        }
        PreprocTokenProvenance::MacroArgument { identity, .. } => {
            Some(MacroTokenIdentity::Argument(identity))
        }
        PreprocTokenProvenance::SourceToken { .. } => None,
    }
}

fn syntax_tokens_for_macro_identities<'tree>(
    root: SyntaxNode<'tree>,
    identities: &[MacroTokenIdentity],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(identity) =
            MacroTokenIdentity::from_syntax_provenance(token.preprocessor_trace_provenance())
        else {
            continue;
        };
        if identities.contains(&identity) && !tokens.contains(&token) {
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
    use syntax::{
        PreprocessorTraceTokenProvenance, SyntaxTree, SyntaxTreeOptions, token::TokenKindExt,
    };

    use super::*;

    #[test]
    fn source_targets_provenance_source_range_hit_test_is_half_open() {
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
    fn source_targets_source_token_range_mismatch_uses_original_syntax_hit() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
        let file_id = FileId(0);
        let provenance_range = TextRange::new(
            parser_range.start() + TextSize::from(1),
            parser_range.end() - TextSize::from(1),
        );
        let hit = test_source_hit(file_id, provenance_range, 0);

        let SourceTargetProviderResult::Resolved(selection) = preproc_provider_result_from_hits(
            root,
            offset,
            &test_precedence,
            vec![hit],
            provenance_range,
        ) else {
            panic!("source-token hit should select by the original syntax token at the offset");
        };

        assert_eq!(selection.range, provenance_range);
        let SourceTargetOrigin::Preproc { hits } = &selection.origin else {
            panic!("preproc provider should produce a preproc-origin selection");
        };
        assert_eq!(hits.len(), 1);
        assert_eq!(selection.tokens.len(), 1);
        assert_eq!(selection.tokens[0].text_range(), Some(parser_range));
        assert_ne!(selection.tokens[0].text_range(), Some(provenance_range));
    }

    #[test]
    fn source_targets_macro_argument_selects_syntax_token_by_trace_identity() {
        let source = r#"`define ID(x) x
module m;
  assign y = `ID(payload_i);
endmodule
"#;
        let parsed = SyntaxTree::from_text_with_options_and_trace(
            source,
            "source",
            "sample/rtl/top.sv",
            &SyntaxTreeOptions::default(),
        );
        let root = parsed.tree.root().expect("test source should parse");
        let token = root
            .elem_preorder()
            .filter_map(|event| match event {
                WalkEvent::Enter(SyntaxElement::Token(token))
                    if token.raw_text().as_bytes() == b"payload_i"
                        && matches!(
                            token.preprocessor_trace_provenance(),
                            PreprocessorTraceTokenProvenance::MacroArgument { .. }
                        ) =>
                {
                    Some(token)
                }
                _ => None,
            })
            .next()
            .expect("expanded source should contain the macro argument token");
        let PreprocessorTraceTokenProvenance::MacroArgument { identity, .. } =
            token.preprocessor_trace_provenance()
        else {
            panic!("payload_i should have macro argument provenance");
        };
        let expected_identity: MacroArgumentTokenIdentity = identity.into();
        let file_id = FileId(0);
        let source_range = source_range(source, "payload_i");
        let source = MappedPreprocSource::RealFile { file_id };
        let hit = PreprocTokenHit {
            expansion: 0,
            call: 0,
            emitted_token: 0,
            display_range: source_range,
            source_range,
            provenance: PreprocTokenProvenance::MacroArgument {
                identity: expected_identity,
                call: 0,
                argument_index: 0,
                source: source.clone(),
                range: source_range,
            },
            target: PreprocSourceTarget::SourceToken { source, range: source_range },
        };

        let tokens =
            syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, &[hit])
                .expect("macro argument identity should resolve to a parsed syntax token");

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
        assert_eq!(
            MacroTokenIdentity::from_syntax_provenance(tokens[0].preprocessor_trace_provenance()),
            Some(MacroTokenIdentity::Argument(expected_identity))
        );
    }

    #[test]
    fn source_targets_preproc_owned_unresolved_does_not_use_normal_syntax_fallback() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        assert!(
            matches!(
                normal_syntax_source_target_at_offset(root, offset, &test_precedence),
                SourceTargetProviderResult::Resolved(_)
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
            SourceTargetProviderResult::Blocked(SourceTargetBlock {
                domain: SourceTargetDomain::Preproc,
                reason: SourceTargetBlockReason::Unavailable,
                ..
            })
        ));
    }

    #[test]
    fn source_targets_normal_syntax_path_still_selects_non_preproc_offsets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        let SourceTargetProviderResult::Resolved(selection) =
            normal_syntax_source_target_at_offset(root, offset, &test_precedence)
        else {
            panic!("normal syntax token expected");
        };

        assert!(matches!(selection.origin, SourceTargetOrigin::NormalSyntax));
        assert_eq!(selection.range, parser_range);
        assert_eq!(selection.tokens.len(), 1);
    }

    #[test]
    fn source_targets_macro_provenance_gate_skips_plain_identifiers() {
        let text = "module m; wire payload_i; endmodule\n";

        assert!(!source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
    }

    #[test]
    fn source_targets_macro_provenance_gate_keeps_macro_names_and_arguments() {
        let text = "module m; wire `MAKE_DECL(payload_i); endmodule\n";

        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "`MAKE_DECL")));
        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
    }

    #[test]
    fn source_targets_macro_provenance_gate_keeps_outer_arguments_after_nested_macros() {
        let text = "assign y = `OUTER(a, `INNER(b), payload_i);\n";

        assert!(source_macro_invocation_may_cover_offset(text, offset(text, "payload_i")));
    }

    #[test]
    fn source_targets_dedups_preproc_hits_for_same_semantic_target() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
        let file_id = FileId(0);
        let hits = vec![
            test_source_hit(file_id, parser_range, 0),
            test_source_hit(file_id, parser_range, 1),
        ];

        let SourceTargetProviderResult::Resolved(selection) =
            preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("same semantic target should dedup to one available preproc hit");
        };

        let SourceTargetOrigin::Preproc { hits } = selection.origin else {
            panic!("preproc provider should produce a preproc-origin selection");
        };
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn source_targets_reports_ambiguous_preproc_hits_for_conflicting_targets() {
        let (root, offset, parser_range) =
            root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
        let file_id = FileId(0);
        let first = TextRange::new(parser_range.start(), parser_range.start() + TextSize::from(4));
        let second = TextRange::new(parser_range.start() + TextSize::from(1), parser_range.end());
        let hits = vec![test_source_hit(file_id, first, 0), test_source_hit(file_id, second, 1)];

        let SourceTargetProviderResult::Blocked(SourceTargetBlock {
            reason: SourceTargetBlockReason::Ambiguous { hits },
            ..
        }) = preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
        else {
            panic!("conflicting preproc targets should be ambiguous");
        };

        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn source_target_request_cache_reuses_provenance_lookup_for_repeated_reference_hits() {
        let mut cache = SourceTargetRequestCache::default();
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

    fn source_range(text: &str, needle: &str) -> TextRange {
        let start = text.find(needle).expect("needle should exist");
        TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32))
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
            target: PreprocSourceTarget::SourceToken { source, range },
        }
    }

    fn preproc_provider_result_from_hits<'tree>(
        root: SyntaxNode<'tree>,
        offset: TextSize,
        precedence: &impl Fn(TokenKind) -> usize,
        hits: Vec<PreprocTokenHit>,
        fallback_range: TextRange,
    ) -> SourceTargetProviderResult<'tree> {
        let mut unique_hits = Vec::new();
        for hit in hits {
            if hit.source_range.contains(offset) {
                push_unique_preproc_hit(&mut unique_hits, hit);
            }
        }
        if unique_hits.is_empty() {
            return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(
                fallback_range,
            ));
        }
        let range =
            covering_range(&unique_hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
                .unwrap_or(fallback_range);
        if unique_hits.len() > 1 {
            return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_ambiguous(
                range,
                unique_hits,
            ));
        }
        let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &unique_hits)
        else {
            return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(
                range,
            ));
        };
        SourceTargetProviderResult::Resolved(SourceTarget::preproc(range, unique_hits, tokens))
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
