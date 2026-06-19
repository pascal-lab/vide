use hir::{
    base_db::source_db::SourceDb,
    db::HirDb,
    hir_def::macro_file::{ExpansionSourceHit, MacroFileId, Origin, SourceEmittedTokenId},
};
use syntax::{SyntaxElement, SyntaxNode, SyntaxTokenWithParent, TokenKind, WalkEvent};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use super::{
    PreprocTokenHit, SourceTarget, SourceTargetBlock, SourceTargetProviderResult,
    SourceTargetRequestCache, SourceTargetResolution, covering_range,
    macro_gate::source_macro_invocation_may_cover_offset, normal_syntax_source_target_at_offset,
};
use crate::db::root_db::RootDb;

pub(super) fn preproc_source_target_at_offset<'tree>(
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

    let macro_files = cache.macro_files_at_offset(db, file_id, offset);
    if macro_files.is_empty() {
        return SourceTargetProviderResult::NotApplicable;
    }

    match preproc_hits_at_offset(db, &macro_files, file_id, offset) {
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
    db: &RootDb,
    macro_files: &[MacroFileId],
    file_id: FileId,
    offset: TextSize,
) -> PreprocHitLookup {
    let mut hits = Vec::new();
    for (expansion_index, macro_file) in macro_files.iter().enumerate() {
        let expansion = db.macro_expansion(*macro_file);
        for source_hit in expansion.source_map.source_hits(file_id, offset) {
            let Some(hit) = preproc_hit_for_source_hit(db, expansion_index, source_hit) else {
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

fn preproc_hit_for_source_hit(
    db: &dyn HirDb,
    expansion: usize,
    source_hit: ExpansionSourceHit,
) -> Option<PreprocTokenHit> {
    let call = origin_call(db, &source_hit.origin).unwrap_or(0);
    Some(PreprocTokenHit {
        expansion,
        call,
        emitted_token: source_hit.emitted_token,
        display_range: source_hit.range,
        source_range: source_hit.range,
        origin: source_hit.origin,
    })
}

fn origin_call(db: &dyn HirDb, origin: &Origin) -> Option<usize> {
    let call = match origin {
        Origin::File { .. } => return None,
        Origin::MacroBody { call, .. }
        | Origin::MacroArg { call, .. }
        | Origin::TokenPaste { call }
        | Origin::Stringify { call }
        | Origin::Builtin { call, .. } => call,
    };
    usize::try_from(db.lookup_intern_macro_call(*call).trace_call.0).ok()
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

pub(super) fn syntax_tokens_for_preproc_hit<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let emitted_tokens = hits.iter().filter_map(macro_emitted_token_for_hit).collect::<Vec<_>>();
    if !emitted_tokens.is_empty() {
        return syntax_tokens_for_macro_emitted_tokens(root, &emitted_tokens);
    }

    normal_syntax_source_target_at_offset(root, offset, precedence)
        .into_resolution()
        .and_then(SourceTargetResolution::resolved)
        .map(SourceTarget::into_tokens)
}

fn macro_emitted_token_for_hit(hit: &PreprocTokenHit) -> Option<SourceEmittedTokenId> {
    (!matches!(hit.origin, Origin::File { .. })).then_some(hit.emitted_token)
}

fn syntax_tokens_for_macro_emitted_tokens<'tree>(
    root: SyntaxNode<'tree>,
    emitted_tokens: &[SourceEmittedTokenId],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(token_id) = syntax_token_emitted_token_id(&token) else {
            continue;
        };
        if emitted_tokens.contains(&token_id) && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn syntax_token_emitted_token_id(
    token: &SyntaxTokenWithParent<'_>,
) -> Option<SourceEmittedTokenId> {
    token
        .preprocessor_trace_emitted_token()
        .emitted_token_index
        .and_then(|index| usize::try_from(index).ok())
        .map(SourceEmittedTokenId::new)
}
