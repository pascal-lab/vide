//! Preprocessor-owned source target resolution.
//!
//! Offsets inside macro definitions, parameters, references, includes, and
//! macro-emitted tokens resolve here. The provider returns a
//! [`SourceTargetProviderResult`] without a file anchor; the caller attaches
//! the file id and produces the final [`TargetResolution`].

use preproc_expand::{
    context::{MacroContext, macro_context_at},
    db::PreprocDb,
    macro_file::{ExpansionSourceHit, MacroFileId, Origin, SourceEmittedTokenId},
};
use rustc_hash::FxHashMap;
use syntax::{SyntaxElement, SyntaxNode, SyntaxTokenWithParent, TokenKind, WalkEvent};
use utils::line_index::{TextRange, TextSize, covering_range};
use vfs::FileId;

use super::{
    SemanticTarget, SourceTarget, TargetAlternatives, TargetAmbiguityReason, TargetCandidate,
    TargetResolution, normal_syntax_source_target_at_offset, source_capabilities,
};

/// Emitted-token id to syntax tokens for one parse tree, built with a single
/// tree walk. Macro-emitted tokens share the call-site display range, so
/// positional lookups cannot enumerate them; the preprocessor trace id is the
/// only stable token identity. One id can map to several tokens (a macro can
/// emit the same argument more than once), so every copy is kept.
///
/// Callers that resolve many offsets of one tree (the semantic index build)
/// construct this once and share it across every resolution instead of
/// re-walking the tree per token.
pub(crate) type EmittedTokenIndex<'tree> =
    FxHashMap<SourceEmittedTokenId, Vec<SyntaxTokenWithParent<'tree>>>;

pub(crate) fn emit_token_index<'tree>(root: SyntaxNode<'tree>) -> EmittedTokenIndex<'tree> {
    let mut index = EmittedTokenIndex::default();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        if let Some(emitted_token) = syntax_token_emitted_token_id(&token) {
            index.entry(emitted_token).or_default().push(token);
        }
    }
    index
}

/// A preprocessor source hit: one macro-emitted token (or file range) that
/// the caret offset overlaps, with its trace identity and origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreprocTokenHit {
    pub emitted_token: SourceEmittedTokenId,
    pub source_range: TextRange,
    pub origin: Origin,
}

/// The preproc provider's result. `None` means the offset is not
/// preprocessor-owned and the caller falls back to plain syntax resolution.
pub(super) fn preproc_source_target_at_offset<'tree>(
    db: &dyn PreprocDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    emitted: Option<&EmittedTokenIndex<'tree>>,
) -> Option<TargetResolution<'tree>> {
    let MacroContext::Invocation { macro_files } = macro_context_at(db, file_id, offset) else {
        return None;
    };

    match preproc_hits_at_offset(db, &macro_files, file_id, offset) {
        PreprocHitLookup::Available { range, hits } => {
            let Some(tokens) =
                syntax_tokens_for_preproc_hit(root, offset, precedence, emitted, &hits)
            else {
                return Some(TargetResolution::Blocked);
            };
            Some(TargetResolution::Resolved(TargetCandidate::new(
                SemanticTarget::Source(SourceTarget::preproc(range, tokens)),
                source_capabilities(),
            )))
        }
        PreprocHitLookup::Unavailable => Some(TargetResolution::Blocked),
        PreprocHitLookup::Ambiguous { range, hits } => Some(
            ambiguous_preproc_source_targets(root, offset, precedence, emitted, range, hits)
                .map_or(TargetResolution::Blocked, |(reason, candidates)| {
                    TargetResolution::Ambiguous(TargetAlternatives { reason, candidates })
                }),
        ),
    }
}

enum PreprocHitLookup {
    Available { range: TextRange, hits: Vec<PreprocTokenHit> },
    Unavailable,
    Ambiguous { range: TextRange, hits: Vec<PreprocTokenHit> },
}

fn preproc_hits_at_offset(
    db: &dyn PreprocDb,
    macro_files: &[MacroFileId],
    file_id: FileId,
    offset: TextSize,
) -> PreprocHitLookup {
    let mut hits = Vec::new();
    for macro_file in macro_files {
        let expansion = db.macro_expansion(*macro_file);
        for source_hit in expansion.value.source_map.source_hits(file_id, offset) {
            let Some(hit) = preproc_hit_for_source_hit(source_hit) else {
                continue;
            };
            push_unique_preproc_hit(&mut hits, hit);
        }
    }

    if hits.is_empty() {
        return PreprocHitLookup::Unavailable;
    }

    let range = covering_range(&hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
        .unwrap_or_else(|| TextRange::empty(offset));
    match hits.len() {
        0 => unreachable!(),
        _ if hits_have_one_origin(&hits) => PreprocHitLookup::Available { range, hits },
        _ => PreprocHitLookup::Ambiguous { range, hits },
    }
}

fn preproc_hit_for_source_hit(source_hit: ExpansionSourceHit) -> Option<PreprocTokenHit> {
    Some(PreprocTokenHit {
        emitted_token: source_hit.emitted_token,
        source_range: source_hit.range,
        origin: source_hit.origin,
    })
}

pub(super) fn push_unique_preproc_hit(hits: &mut Vec<PreprocTokenHit>, hit: PreprocTokenHit) {
    if hits.contains(&hit) {
        return;
    }
    hits.push(hit);
}

fn hits_have_one_origin(hits: &[PreprocTokenHit]) -> bool {
    let Some(first) = hits.first() else {
        return false;
    };
    hits.iter().all(|hit| hit.origin == first.origin)
}

/// Projects conflicting preproc hits to one candidate per origin. Returns
/// `None` when the hits cannot be projected to syntax tokens or there is
/// only one origin group.
pub(super) fn ambiguous_preproc_source_targets<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    emitted: Option<&EmittedTokenIndex<'tree>>,
    range: TextRange,
    hits: Vec<PreprocTokenHit>,
) -> Option<(TargetAmbiguityReason, Vec<TargetCandidate<'tree>>)> {
    let hit_count = hits.len();
    let groups = group_preproc_hits_by_origin(hits);
    if groups.len() <= 1 {
        return None;
    }

    let mut candidates = Vec::with_capacity(groups.len());
    for group in groups {
        let group_range =
            covering_range(&group.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
                .unwrap_or(range);
        let tokens = syntax_tokens_for_preproc_hit(root, offset, precedence, emitted, &group)?;
        candidates.push(TargetCandidate::new(
            SemanticTarget::Source(SourceTarget::preproc(group_range, tokens)),
            source_capabilities(),
        ));
    }

    Some((TargetAmbiguityReason::PreprocHits { hit_count }, candidates))
}

fn group_preproc_hits_by_origin(hits: Vec<PreprocTokenHit>) -> Vec<Vec<PreprocTokenHit>> {
    let mut groups: Vec<Vec<PreprocTokenHit>> = Vec::new();
    for hit in hits {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.first().is_some_and(|candidate| candidate.origin == hit.origin))
        {
            group.push(hit);
        } else {
            groups.push(vec![hit]);
        }
    }
    groups
}

pub(super) fn syntax_tokens_for_preproc_hit<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    emitted: Option<&EmittedTokenIndex<'tree>>,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    if hits.iter().any(|hit| macro_emitted_token_for_hit(hit).is_some()) {
        return syntax_tokens_for_macro_emitted_tokens(root, emitted, hits);
    }

    normal_syntax_source_target_at_offset(root, offset, precedence).map(SourceTarget::into_tokens)
}

fn macro_emitted_token_for_hit(hit: &PreprocTokenHit) -> Option<SourceEmittedTokenId> {
    (!matches!(hit.origin, Origin::File { .. })).then_some(hit.emitted_token)
}

/// Resolves the syntax tokens of a macro-emitted source hit.
///
/// Macro-emitted tokens report the call-site display range rather than a
/// physical position, so the tree cannot be queried by offset; the
/// preprocessor trace id is the only stable token identity. The tree is
/// walked once to index tokens by trace id, then each hit is a hash lookup.
/// One trace id can map to several tokens (a macro can emit the same
/// argument more than once), so the index stores every copy.
fn syntax_tokens_for_macro_emitted_tokens<'tree>(
    root: SyntaxNode<'tree>,
    emitted: Option<&EmittedTokenIndex<'tree>>,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let index = match emitted {
        Some(index) => index,
        None => &emit_token_index(root),
    };

    let mut tokens = Vec::new();
    let mut missing = Vec::new();
    for hit in hits {
        let Some(emitted_token) = macro_emitted_token_for_hit(hit) else {
            continue;
        };
        if let Some(copies) = index.get(&emitted_token) {
            tokens.extend_from_slice(copies);
        } else {
            missing.push(emitted_token);
        }
    }
    if !missing.is_empty() {
        // A macro-emitted hit whose trace id is absent from the lookup tree
        // means the hit belongs to a different preprocessor trace (issue
        // #327 cross-model scenario). Keep the miss visible instead of
        // silently dropping it.
        tracing::warn!(
            missing = ?missing,
            "macro-emitted token ids not found in the lookup tree"
        );
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn syntax_token_emitted_token_id(
    token: &SyntaxTokenWithParent<'_>,
) -> Option<SourceEmittedTokenId> {
    token
        .preprocessor_trace_emitted_token_index()
        .and_then(|index| usize::try_from(index).ok())
        .map(SourceEmittedTokenId::new)
}
