use hir::{
    base_db::source_db::SourceDb,
    db::HirDb,
    hir_def::macro_file::{ExpansionSourceHit, MacroCallId, MacroCallLoc, MacroFileId, Origin},
};
use smol_str::ToSmolStr;
use syntax::{
    SourceBufferRange, SyntaxElement, SyntaxNode, SyntaxTokenWithParent, TokenKind, WalkEvent,
    preproc::{MacroCallId as TraceMacroCallId, TokenOrigin},
};
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
            let Some(tokens) =
                syntax_tokens_for_preproc_hit(db, file_id, root, offset, precedence, &hits)
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
        1 => PreprocHitLookup::Available { range, hits },
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
        emitted_token: source_hit.expanded_token_index,
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
    if hits.iter().any(|existing| existing.origin == hit.origin) {
        return;
    }
    hits.push(hit);
}

pub(super) fn syntax_tokens_for_preproc_hit<'tree>(
    db: &dyn HirDb,
    model_file: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let origins = hits.iter().filter_map(macro_origin_for_hit).collect::<Vec<_>>();
    if !origins.is_empty() {
        return syntax_tokens_for_macro_origins(db, model_file, root, &origins);
    }

    normal_syntax_source_target_at_offset(root, offset, precedence)
        .into_resolution()
        .and_then(SourceTargetResolution::resolved)
        .map(SourceTarget::into_tokens)
}

fn macro_origin_for_hit(hit: &PreprocTokenHit) -> Option<&Origin> {
    (!matches!(hit.origin, Origin::File { .. })).then_some(&hit.origin)
}

fn syntax_tokens_for_macro_origins<'tree>(
    db: &dyn HirDb,
    model_file: FileId,
    root: SyntaxNode<'tree>,
    origins: &[&Origin],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(token_origin) =
            origin_from_token_origin_raw(db, model_file, &token.preprocessor_trace_origin())
        else {
            continue;
        };
        if matches!(token_origin, Origin::File { .. }) {
            continue;
        }
        if origins.iter().any(|origin| **origin == token_origin) && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    (!tokens.is_empty()).then_some(tokens)
}

/// Map a syntax-tree `TokenOrigin` to a hir `Origin` using raw buffer offsets,
/// without consulting a `PreprocSourceMap`.
///
/// This is the inverse used to compare an emitted token against an `Origin`
/// produced earlier by [`Origin::from_token_origin`] when both sides share the
/// same buffer (the common case for single-file expansions). When the call
/// site has a `PreprocSourceMap` available, prefer
/// [`Origin::from_token_origin`] instead.
pub(super) fn origin_from_token_origin_raw(
    db: &dyn HirDb,
    model_file: FileId,
    origin: &TokenOrigin,
) -> Option<Origin> {
    Some(match origin {
        TokenOrigin::Source { token_range } => {
            Origin::File { file: model_file, range: source_buffer_text_range(token_range)? }
        }
        TokenOrigin::MacroBody { call_id, definition_id, body_token_range, .. } => {
            Origin::MacroBody {
                call: macro_call_id(db, model_file, *call_id),
                def: *definition_id,
                body_range: source_buffer_text_range(body_token_range)?,
            }
        }
        TokenOrigin::MacroArgument { call_id, argument_index, argument_token_range, .. } => {
            Origin::MacroArg {
                call: macro_call_id(db, model_file, *call_id),
                arg_index: usize::try_from(*argument_index).ok()?,
                arg_range: source_buffer_text_range(argument_token_range)?,
            }
        }
        TokenOrigin::TokenPaste { call_id, .. } => {
            Origin::TokenPaste { call: macro_call_id(db, model_file, *call_id) }
        }
        TokenOrigin::Stringify { call_id, .. } => {
            Origin::Stringify { call: macro_call_id(db, model_file, *call_id) }
        }
        TokenOrigin::Builtin { name, call_id, .. } if !name.is_empty() => Origin::Builtin {
            call: macro_call_id(db, model_file, *call_id),
            name: name.to_smolstr(),
        },
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => return None,
    })
}

fn macro_call_id(db: &dyn HirDb, model_file: FileId, trace_call: TraceMacroCallId) -> MacroCallId {
    db.intern_macro_call(MacroCallLoc { model_file, trace_call })
}

fn source_buffer_text_range(range: &SourceBufferRange) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.range.start).ok()?),
        TextSize::from(u32::try_from(range.range.end).ok()?),
    ))
}
