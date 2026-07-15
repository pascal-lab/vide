use hir::{db::InternDb, hir_def::macro_file::SourceEmittedTokenId};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxTree, SyntaxTreeOptions, WalkEvent, preproc::TokenOrigin,
    token::TokenKindExt,
};
use utils::line_index::covering_range;

use super::*;

mod cache;
mod macro_gate;

#[test]
fn source_targets_origin_source_range_hit_test_is_half_open() {
    let file_id = FileId::from_raw(0);
    let range = TextRange::new(5.into(), 10.into());
    let origin = Origin::File { file: file_id, range };

    assert!(
        preproc_hit_for_raw_origin(&origin, file_id, 5.into()).is_some(),
        "range start should hit"
    );
    assert!(
        preproc_hit_for_raw_origin(&origin, file_id, 9.into()).is_some(),
        "offset before range end should hit"
    );
    assert!(
        preproc_hit_for_raw_origin(&origin, file_id, 10.into()).is_none(),
        "range end should not hit"
    );
}

#[test]
fn source_targets_source_token_range_mismatch_uses_original_syntax_hit() {
    let (root, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
    let file_id = FileId::from_raw(0);
    let origin_range = TextRange::new(
        parser_range.start() + TextSize::from(1),
        parser_range.end() - TextSize::from(1),
    );
    let hit = test_source_hit(file_id, origin_range, 0);

    let SourceTargetProviderResult::Resolved(selection) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, vec![hit], origin_range)
    else {
        panic!("source-token hit should select by the original syntax token at the offset");
    };

    assert_eq!(selection.range, origin_range);
    let SourceTargetOrigin::Preproc { hits } = &selection.origin else {
        panic!("preproc provider should produce a preproc-origin selection");
    };
    assert_eq!(hits.len(), 1);
    assert_eq!(selection.tokens.len(), 1);
    assert_eq!(selection.tokens[0].text_range(), Some(parser_range));
    assert_ne!(selection.tokens[0].text_range(), Some(origin_range));
}

#[test]
fn source_targets_macro_argument_selects_syntax_token_by_trace_identity() {
    let db = RootDb::new(None);
    let model_file = FileId::from_raw(0);
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
                        token.preprocessor_trace_origin(),
                        TokenOrigin::MacroArgument { .. }
                    ) =>
            {
                Some(token)
            }
            _ => None,
        })
        .next()
        .expect("expanded source should contain the macro argument token");
    let emitted_token = token.preprocessor_trace_emitted_token();
    let expected_origin =
        macro_arg_origin_from_token_origin(&db, model_file, &emitted_token.origin);
    let emitted_token = SourceEmittedTokenId::new(
        usize::try_from(
            emitted_token
                .emitted_token_index
                .expect("syntax token should carry trace emitted-token identity"),
        )
        .unwrap(),
    );
    let source_range = source_range(source, "payload_i");
    let hit = PreprocTokenHit {
        expansion: 0,
        call: 0,
        emitted_token,
        display_range: source_range,
        source_range,
        origin: expected_origin.clone(),
    };

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, &[hit])
            .expect("macro argument origin should resolve to a parsed syntax token");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    assert_eq!(
        tokens[0].preprocessor_trace_emitted_token().emitted_token_index,
        Some(u32::try_from(emitted_token.raw()).unwrap())
    );
}

#[test]
fn source_targets_macro_argument_selects_only_the_hit_emitted_token() {
    let db = RootDb::new(None);
    let model_file = FileId::from_raw(0);
    let source = r#"`define DUP(x) x x
module m;
  assign y = `DUP(payload_i);
endmodule
"#;
    let parsed = SyntaxTree::from_text_with_options_and_trace(
        source,
        "source",
        "sample/rtl/top.sv",
        &SyntaxTreeOptions::default(),
    );
    let root = parsed.tree.root().expect("test source should parse");
    let trace = parsed.preprocessor_trace.expect("trace should be collected");
    let emitted_payloads = trace
        .emitted_tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            token.raw_text == "payload_i"
                && matches!(token.origin, TokenOrigin::MacroArgument { .. })
        })
        .collect::<Vec<_>>();
    assert_eq!(emitted_payloads.len(), 2, "DUP should emit the argument twice");
    let (second_emitted_token, second_emitted_payload) = emitted_payloads[1];
    let expected_origin =
        macro_arg_origin_from_token_origin(&db, model_file, &second_emitted_payload.origin);
    let expected_tokens = root
        .elem_preorder()
        .filter_map(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token))
                if token.raw_text().as_bytes() == b"payload_i"
                    && matches!(
                        token.preprocessor_trace_origin(),
                        TokenOrigin::MacroArgument { .. }
                    ) =>
            {
                Some(token)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(expected_tokens.len(), 2, "expanded syntax should contain both argument copies");
    let source_range = source_range(source, "payload_i");
    let hit = PreprocTokenHit {
        expansion: 0,
        call: 0,
        emitted_token: SourceEmittedTokenId::new(second_emitted_token),
        display_range: source_range,
        source_range,
        origin: expected_origin,
    };

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, &[hit])
            .expect("macro argument emitted token should resolve to a parsed syntax token");

    assert_eq!(tokens, vec![expected_tokens[1]]);
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

    let lookup =
        preproc_provider_result_from_hits(root, offset, &test_precedence, Vec::new(), parser_range);
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
fn source_targets_same_origin_hits_are_available_without_dropping_emitted_tokens() {
    let (root, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
    let file_id = FileId::from_raw(0);
    let hits =
        vec![test_source_hit(file_id, parser_range, 0), test_source_hit(file_id, parser_range, 1)];

    let SourceTargetProviderResult::Resolved(selection) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
    else {
        panic!("same-origin hits should remain available");
    };

    let SourceTargetOrigin::Preproc { hits } = selection.origin else {
        panic!("preproc provider should produce a preproc-origin selection");
    };
    assert_eq!(hits.len(), 2);
}

#[test]
fn source_targets_reports_ambiguous_preproc_hits_for_conflicting_targets() {
    let (root, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
    let file_id = FileId::from_raw(0);
    let first = TextRange::new(parser_range.start(), parser_range.start() + TextSize::from(4));
    let second = TextRange::new(parser_range.start() + TextSize::from(1), parser_range.end());
    let hits = vec![test_source_hit(file_id, first, 0), test_source_hit(file_id, second, 1)];

    let SourceTargetProviderResult::Ambiguous(alternatives) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
    else {
        panic!("conflicting preproc targets should produce alternatives");
    };

    assert_eq!(alternatives.reason, SourceTargetAmbiguity::PreprocHits { hit_count: 2 });
    assert_eq!(alternatives.targets.len(), 2);
}

fn root_and_offset<'tree>(
    text: &str,
    needle: &str,
    delta: u32,
) -> (SyntaxNode<'tree>, TextSize, TextRange) {
    let tree = Box::leak(Box::new(SyntaxTree::from_text(text, "test", "test.sv")));
    let root = tree.root().expect("test source should parse");
    let start = text.find(needle).expect("needle should exist");
    let range =
        TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32));
    (root, range.start() + TextSize::from(delta), range)
}

pub(super) fn offset(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).expect("needle should exist")).unwrap())
}

fn source_range(text: &str, needle: &str) -> TextRange {
    let start = text.find(needle).expect("needle should exist");
    TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32))
}

fn test_source_hit(file_id: FileId, range: TextRange, emitted_token: usize) -> PreprocTokenHit {
    let origin = Origin::File { file: file_id, range };
    PreprocTokenHit {
        expansion: 0,
        call: 0,
        emitted_token: SourceEmittedTokenId::new(emitted_token),
        display_range: range,
        source_range: range,
        origin,
    }
}

fn macro_arg_origin_from_token_origin(
    db: &RootDb,
    model_file: FileId,
    origin: &TokenOrigin,
) -> Origin {
    let TokenOrigin::MacroArgument { call_id, argument_index, argument_token_range, .. } = origin
    else {
        panic!("macro argument origin expected");
    };
    Origin::MacroArg {
        call: db.intern_macro_call(hir::hir_def::macro_file::MacroCallLoc {
            model_file,
            trace_call: *call_id,
        }),
        arg_index: usize::try_from(*argument_index).unwrap(),
        arg_range: TextRange::new(
            TextSize::from(u32::try_from(argument_token_range.range.start).unwrap()),
            TextSize::from(u32::try_from(argument_token_range.range.end).unwrap()),
        ),
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
    let range = covering_range(&unique_hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
        .unwrap_or(fallback_range);
    let has_conflicting_origin = unique_hits
        .first()
        .is_some_and(|first| unique_hits.iter().any(|hit| hit.origin != first.origin));
    if has_conflicting_origin {
        let block_hits = unique_hits.clone();
        return ambiguous_preproc_source_targets(root, offset, precedence, range, unique_hits)
            .map(SourceTargetProviderResult::Ambiguous)
            .unwrap_or_else(|| {
                SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_ambiguous(
                    range, block_hits,
                ))
            });
    }
    let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, &unique_hits) else {
        return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_unavailable(range));
    };
    SourceTargetProviderResult::Resolved(SourceTarget::preproc(range, unique_hits, tokens))
}

fn preproc_hit_for_raw_origin(
    origin: &Origin,
    file_id: FileId,
    offset: TextSize,
) -> Option<TextRange> {
    let (source_file, range) = match origin {
        Origin::File { file, range } => (*file, *range),
        _ => return None,
    };
    (source_file == file_id && range.contains(offset)).then_some(range)
}

fn test_precedence(kind: TokenKind) -> usize {
    usize::from(kind.name_like())
}
