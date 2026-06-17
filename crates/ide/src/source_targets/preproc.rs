use hir::{
    base_db::source_db::SourceDb,
    db::HirDb,
    hir_def::macro_file::{ExpansionSourceHit, MacroFileId, Origin},
};
use smol_str::ToSmolStr;
use syntax::{
    SourceBufferRange, SyntaxElement, SyntaxNode, SyntaxTokenWithParent, TokenKind, WalkEvent,
    preproc::TokenOrigin,
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
            let Some(hit) = preproc_hit_for_source_hit(expansion_index, source_hit) else {
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
    expansion: usize,
    source_hit: ExpansionSourceHit,
) -> Option<PreprocTokenHit> {
    let call = origin_call(&source_hit.origin).unwrap_or(0);
    Some(PreprocTokenHit {
        expansion,
        call,
        emitted_token: source_hit.expanded_token_index,
        display_range: source_hit.range,
        source_range: source_hit.range,
        origin: source_hit.origin,
    })
}

fn origin_call(origin: &Origin) -> Option<usize> {
    let call = match origin {
        Origin::File { .. } => return None,
        Origin::MacroBody { call, .. }
        | Origin::MacroArg { call, .. }
        | Origin::TokenPaste { call }
        | Origin::Stringify { call }
        | Origin::Builtin { call, .. } => call,
    };
    usize::try_from(call.0).ok()
}

pub(super) fn push_unique_preproc_hit(hits: &mut Vec<PreprocTokenHit>, hit: PreprocTokenHit) {
    if hits.iter().any(|existing| existing.origin == hit.origin) {
        return;
    }
    hits.push(hit);
}

pub(super) fn syntax_tokens_for_preproc_hit<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: &impl Fn(TokenKind) -> usize,
    hits: &[PreprocTokenHit],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let origins = hits.iter().filter_map(macro_origin_for_hit).collect::<Vec<_>>();
    if !origins.is_empty() {
        return syntax_tokens_for_macro_origins(root, &origins);
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
    root: SyntaxNode<'tree>,
    origins: &[&Origin],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(origin) = origin_from_syntax_provenance(token.preprocessor_trace_provenance())
        else {
            continue;
        };
        if origins.contains(&&origin) && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    (!tokens.is_empty()).then_some(tokens)
}

pub(super) fn origin_from_syntax_provenance(origin: TokenOrigin) -> Option<Origin> {
    match origin {
        TokenOrigin::Source { .. } => None,
        TokenOrigin::MacroBody { identity, body_token_range, .. } => Some(Origin::MacroBody {
            call: identity.call_id,
            def: identity.definition_id,
            body_range: source_buffer_text_range(&body_token_range)?,
        }),
        TokenOrigin::MacroArgument { identity, argument_token_range, .. } => {
            Some(Origin::MacroArg {
                call: identity.call_id,
                arg_index: usize::try_from(identity.argument_index).ok()?,
                arg_range: source_buffer_text_range(&argument_token_range)?,
            })
        }
        TokenOrigin::TokenPaste { identity } => Some(Origin::TokenPaste { call: identity.call_id }),
        TokenOrigin::Stringification { identity } => {
            Some(Origin::Stringify { call: identity.call_id })
        }
        TokenOrigin::Builtin { name, identity } if !name.is_empty() => {
            Some(Origin::Builtin { call: identity.call_id, name: name.to_smolstr() })
        }
        TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
    }
}

fn source_buffer_text_range(range: &SourceBufferRange) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.range.start).ok()?),
        TextSize::from(u32::try_from(range.range.end).ok()?),
    ))
}
