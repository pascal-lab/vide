use std::collections::HashMap;

use preproc_expand::{
    context::{MacroContext, macro_context_at},
    db::PreprocDb,
    macro_file::{ExpansionSourceHit, MacroFileId, Origin, SourceEmittedTokenId},
};
use syntax::{SyntaxElement, SyntaxNode, SyntaxTokenWithParent, TokenKind, WalkEvent};
use utils::line_index::{TextRange, TextSize, covering_range};
use vfs::FileId;

use super::{
    PreprocTokenHit, SourceTarget, SourceTargetAlternatives, SourceTargetBlock,
    SourceTargetProviderResult, SourceTargetResolution, normal_syntax_source_target_at_offset,
};

pub(super) fn preproc_source_target_at_offset<'tree>(
    db: &dyn PreprocDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
) -> SourceTargetProviderResult<'tree> {
    let MacroContext::Invocation { macro_files } = macro_context_at(db, file_id, offset) else {
        return SourceTargetProviderResult::NotApplicable;
    };

    match preproc_hits_at_offset(db, &macro_files, file_id, offset) {
        PreprocHitLookup::Available { range, hits } => {
            let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &hits)
            else {
                return SourceTargetProviderResult::Blocked(
                    SourceTargetBlock::preproc_unavailable(range),
                );
            };
            SourceTargetProviderResult::Resolved(SourceTarget::preproc(range, tokens))
        }
        PreprocHitLookup::Unavailable { range } => {
            SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(range))
        }
        PreprocHitLookup::Ambiguous { range, hits } => {
            let block_hits = hits.clone();
            ambiguous_preproc_source_targets(root, offset, precedence, range, hits)
                .map(SourceTargetProviderResult::Ambiguous)
                .unwrap_or_else(|| {
                    SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_ambiguous(
                        range, block_hits,
                    ))
                })
        }
    }
}

enum PreprocHitLookup {
    Available { range: TextRange, hits: Vec<PreprocTokenHit> },
    Unavailable { range: TextRange },
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
        return PreprocHitLookup::Unavailable { range: TextRange::empty(offset) };
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

pub(super) fn ambiguous_preproc_source_targets<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    range: TextRange,
    hits: Vec<PreprocTokenHit>,
) -> Option<SourceTargetAlternatives<'tree>> {
    let hit_count = hits.len();
    let groups = group_preproc_hits_by_origin(hits);
    if groups.len() <= 1 {
        return None;
    }

    let mut targets = Vec::with_capacity(groups.len());
    for group in groups {
        let group_range =
            covering_range(&group.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
                .unwrap_or(range);
        let tokens = syntax_tokens_for_preproc_hit(root, offset, precedence, &group)?;
        targets.push(SourceTarget::preproc(group_range, tokens));
    }

    Some(SourceTargetAlternatives::preproc_ambiguous(range, hit_count, targets))
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
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    if hits.iter().any(|hit| macro_emitted_token_for_hit(hit).is_some()) {
        return syntax_tokens_for_macro_emitted_tokens(root, hits);
    }

    normal_syntax_source_target_at_offset(root, offset, precedence)
        .into_resolution()
        .and_then(SourceTargetResolution::resolved)
        .map(SourceTarget::into_tokens)
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
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut index: HashMap<SourceEmittedTokenId, Vec<SyntaxTokenWithParent<'tree>>> =
        HashMap::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        if let Some(emitted_token) = syntax_token_emitted_token_id(&token) {
            index.entry(emitted_token).or_default().push(token);
        }
    }

    let mut tokens = Vec::new();
    for hit in hits {
        let Some(emitted_token) = macro_emitted_token_for_hit(hit) else {
            continue;
        };
        if let Some(copies) = index.get(&emitted_token) {
            tokens.extend_from_slice(copies);
        }
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
