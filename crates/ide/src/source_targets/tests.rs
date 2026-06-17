use syntax::{
    SyntaxElement, SyntaxNode, SyntaxTree, SyntaxTreeOptions, WalkEvent, preproc::TokenOrigin,
    token::TokenKindExt,
};

use super::*;

mod cache;
mod macro_gate;

#[test]
fn source_targets_origin_source_range_hit_test_is_half_open() {
    let file_id = FileId(0);
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
    let file_id = FileId(0);
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
                        TokenOrigin::MacroArgument { .. }
                    ) =>
            {
                Some(token)
            }
            _ => None,
        })
        .next()
        .expect("expanded source should contain the macro argument token");
    let expected_origin = origin_from_syntax_provenance(token.preprocessor_trace_provenance())
        .expect("payload_i should have macro argument provenance");
    let source_range = source_range(source, "payload_i");
    let hit = PreprocTokenHit {
        expansion: 0,
        call: 0,
        emitted_token: 0,
        display_range: source_range,
        source_range,
        origin: expected_origin.clone(),
    };

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, &[hit])
            .expect("macro argument identity should resolve to a parsed syntax token");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    assert_eq!(
        origin_from_syntax_provenance(tokens[0].preprocessor_trace_provenance()),
        Some(expected_origin)
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
fn source_targets_dedups_preproc_hits_for_same_semantic_target() {
    let (root, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
    let file_id = FileId(0);
    let hits =
        vec![test_source_hit(file_id, parser_range, 0), test_source_hit(file_id, parser_range, 1)];

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
        emitted_token,
        display_range: range,
        source_range: range,
        origin,
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
    if unique_hits.len() > 1 {
        return SourceTargetProviderResult::Blocked(SourceTargetBlock::preproc_ambiguous(
            range,
            unique_hits,
        ));
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
