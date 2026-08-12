use base_db::{change::Change, source_root::SourceRoot};
use hir_semantics::semantics::Semantics;
use preproc_expand::macro_file::{Origin, SourceEmittedTokenId};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxTree, SyntaxTreeOptions, WalkEvent, preproc::TokenOrigin,
    token::TokenKindExt,
};
use utils::line_index::covering_range;
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use super::{
    preproc::{
        PreprocTokenHit, ambiguous_preproc_source_targets, push_unique_preproc_hit,
        syntax_tokens_for_preproc_hit,
    },
    *,
};
use crate::{
    analysis_host::AnalysisHost, db::root_db::RootDb, token::name_precedence as token_precedence,
};

mod bench_context;

#[test]
fn source_token_target_is_complete_and_source_origin() {
    let (host, file_id, offset, range) =
        setup("module m; wire payload_i; endmodule\n", "payload_i");
    let sema = Semantics::new(host.raw_db());
    let parsed = sema.parse_file(file_id);
    let root = parsed.root().expect("test source should parse");

    let resolution =
        resolve_semantic_target(host.raw_db(), file_id, offset, Some(root), token_precedence);
    assert!(matches!(
        resolution.clone().unique_for_intent(TargetIntent::Describe),
        Some(SemanticTarget::Source(_))
    ));
    assert!(matches!(
        resolution.clone().unique_for_intent(TargetIntent::Rename),
        Some(SemanticTarget::Source(_))
    ));

    let TargetResolution::Resolved(target) = resolution else {
        panic!("source token should resolve");
    };

    assert!(target.capabilities.contains(TargetCapability::DESCRIBE));
    let SemanticTarget::Source(target) = target.target else {
        panic!("source token should resolve as source target");
    };
    assert_eq!(target.range, range);
}

#[test]
fn source_token_range_mismatch_uses_original_syntax_hit() {
    let (tree, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
    let root = tree.root();
    let file_id = FileId::from_raw(0);
    let origin_range = TextRange::new(
        parser_range.start() + TextSize::from(1),
        parser_range.end() - TextSize::from(1),
    );
    let hit = test_source_hit(file_id, origin_range, 0);

    let TargetResolution::Resolved(selection) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, vec![hit], origin_range)
    else {
        panic!("source-token hit should select by the original syntax token at the offset");
    };
    let SemanticTarget::Source(selection) = selection.target else {
        panic!("source-token hit should resolve as a source target");
    };

    assert_eq!(selection.range, origin_range);
    assert_eq!(selection.tokens.len(), 1);
    assert_eq!(selection.tokens[0].text_range(), Some(parser_range));
    assert_ne!(selection.tokens[0].text_range(), Some(origin_range));
}

#[test]
fn macro_argument_selects_syntax_token_by_trace_identity() {
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
    let root = parsed.tree.root();
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
    let hit = PreprocTokenHit { emitted_token, source_range, origin: expected_origin.clone() };

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, None, &[hit])
            .expect("macro argument origin should resolve to a parsed syntax token");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    assert_eq!(
        tokens[0].preprocessor_trace_emitted_token().emitted_token_index,
        Some(u32::try_from(emitted_token.raw()).unwrap())
    );
}

#[test]
fn macro_argument_selects_only_the_hit_emitted_token() {
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
    let root = parsed.tree.root();
    let trace = parsed.preprocessor_trace;
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
        emitted_token: SourceEmittedTokenId::new(second_emitted_token),
        source_range,
        origin: expected_origin,
    };

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, None, &[hit])
            .expect("macro argument emitted token should resolve to a parsed syntax token");

    assert_eq!(tokens, vec![expected_tokens[1]]);
}

#[test]
fn macro_hit_miss_does_not_block_resolvable_hits() {
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
    let root = parsed.tree.root();
    let token = root
        .elem_preorder()
        .filter_map(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token))
                if token.raw_text().as_bytes() == b"payload_i" =>
            {
                Some(token)
            }
            _ => None,
        })
        .next()
        .expect("expanded source should contain the macro argument token");
    let emitted = token.preprocessor_trace_emitted_token();
    let real_id = SourceEmittedTokenId::new(
        usize::try_from(emitted.emitted_token_index.expect("trace id")).unwrap(),
    );
    let origin = macro_arg_origin_from_token_origin(&db, model_file, &emitted.origin);
    let source_range = source_range(source, "payload_i");
    let real_hit = PreprocTokenHit { emitted_token: real_id, source_range, origin: origin.clone() };
    // A stale or cross-trace hit whose trace id does not exist in this tree
    // must not discard the tokens the resolvable hits produced.
    let bogus_hit =
        PreprocTokenHit { emitted_token: SourceEmittedTokenId::new(999_999), source_range, origin };

    let tokens = syntax_tokens_for_preproc_hit(
        root,
        source_range.start(),
        &test_precedence,
        None,
        &[real_hit, bogus_hit],
    )
    .expect("resolvable hits should still resolve when another hit misses");

    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
}

#[test]
fn macro_dup_copies_all_resolve() {
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
    let root = parsed.tree.root();
    let trace = parsed.preprocessor_trace;
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
    let origin = macro_arg_origin_from_token_origin(&db, model_file, &emitted_payloads[0].1.origin);
    let source_range = source_range(source, "payload_i");
    let hits = emitted_payloads
        .iter()
        .map(|(index, _)| PreprocTokenHit {
            emitted_token: SourceEmittedTokenId::new(*index),
            source_range,
            origin: origin.clone(),
        })
        .collect::<Vec<_>>();

    let tokens =
        syntax_tokens_for_preproc_hit(root, source_range.start(), &test_precedence, None, &hits)
            .expect("both argument copies should resolve to their syntax tokens");

    assert_eq!(tokens.len(), 2);
    assert!(tokens.iter().all(|token| token.raw_text().as_bytes() == b"payload_i"));
    assert_ne!(tokens[0], tokens[1], "the two copies are distinct tree tokens");
}

#[test]
fn preproc_owned_unresolved_does_not_use_normal_syntax_fallback() {
    let (tree, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
    let root = tree.root();
    assert!(
        normal_syntax_source_target_at_offset(root, offset, &test_precedence).is_some(),
        "test setup must have an ordinary syntax token that fallback could have selected"
    );

    let lookup =
        preproc_provider_result_from_hits(root, offset, &test_precedence, Vec::new(), parser_range);
    assert!(matches!(lookup, TargetResolution::Blocked));
}

#[test]
fn normal_syntax_path_still_selects_non_preproc_offsets() {
    let (tree, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
    let root = tree.root();
    let Some(selection) = normal_syntax_source_target_at_offset(root, offset, &test_precedence)
    else {
        panic!("normal syntax token expected");
    };

    assert_eq!(selection.range, parser_range);
    assert_eq!(selection.tokens.len(), 1);
}

#[test]
fn same_origin_hits_resolve_without_ambiguity() {
    let (tree, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 0);
    let root = tree.root();
    let file_id = FileId::from_raw(0);
    let hits =
        vec![test_source_hit(file_id, parser_range, 0), test_source_hit(file_id, parser_range, 1)];

    let TargetResolution::Resolved(selection) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
    else {
        panic!("same-origin hits should remain available");
    };
    let SemanticTarget::Source(selection) = selection.target else {
        panic!("same-origin hits should resolve as a source target");
    };

    assert_eq!(selection.range, parser_range);
}

#[test]
fn reports_ambiguous_preproc_hits_for_conflicting_targets() {
    let (tree, offset, parser_range) =
        root_and_offset("module m; wire payload_i; endmodule\n", "payload_i", 2);
    let root = tree.root();
    let file_id = FileId::from_raw(0);
    let first = TextRange::new(parser_range.start(), parser_range.start() + TextSize::from(4));
    let second = TextRange::new(parser_range.start() + TextSize::from(1), parser_range.end());
    let hits = vec![test_source_hit(file_id, first, 0), test_source_hit(file_id, second, 1)];

    let TargetResolution::Ambiguous(alternatives) =
        preproc_provider_result_from_hits(root, offset, &test_precedence, hits, parser_range)
    else {
        panic!("conflicting preproc targets should produce alternatives");
    };

    assert_eq!(alternatives.reason, TargetAmbiguityReason::PreprocHits { hit_count: 2 });
    assert_eq!(alternatives.candidates.len(), 2);
}

fn setup(text: &str, needle: &str) -> (AnalysisHost, FileId, TextSize, TextRange) {
    let file_id = FileId::from_raw(0);
    let path = VfsPath::new_virtual_path("/test.sv".to_string());
    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile::create(file_id, text));

    let mut host = AnalysisHost::default();
    host.apply_change(change);

    let start = text.find(needle).expect("needle should exist");
    let range =
        TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32));
    (host, file_id, range.start(), range)
}

fn root_and_offset(text: &str, needle: &str, delta: u32) -> (SyntaxTree, TextSize, TextRange) {
    let tree = SyntaxTree::from_text(text, "test", "test.sv");
    let start = text.find(needle).expect("needle should exist");
    let range =
        TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32));
    (tree, range.start() + TextSize::from(delta), range)
}

fn source_range(text: &str, needle: &str) -> TextRange {
    let start = text.find(needle).expect("needle should exist");
    TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32))
}

fn test_source_hit(file_id: FileId, range: TextRange, emitted_token: usize) -> PreprocTokenHit {
    let origin = Origin::File { file: file_id, range };
    PreprocTokenHit {
        emitted_token: SourceEmittedTokenId::new(emitted_token),
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
        call: preproc_expand::macro_file::MacroCallId::new(
            db,
            preproc_expand::macro_file::MacroCallLoc {
                model_file,
                trace_call: *call_id,
            },
        ),
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
) -> TargetResolution<'tree> {
    let mut unique_hits = Vec::new();
    for hit in hits {
        if hit.source_range.contains(offset) {
            push_unique_preproc_hit(&mut unique_hits, hit);
        }
    }
    if unique_hits.is_empty() {
        return TargetResolution::Blocked;
    }
    let range = covering_range(&unique_hits.iter().map(|hit| hit.source_range).collect::<Vec<_>>())
        .unwrap_or(fallback_range);
    let has_conflicting_origin = unique_hits
        .first()
        .is_some_and(|first| unique_hits.iter().any(|hit| hit.origin != first.origin));
    if has_conflicting_origin {
        return ambiguous_preproc_source_targets(
            root,
            offset,
            precedence,
            None,
            range,
            unique_hits,
        )
        .map_or(TargetResolution::Blocked, |(reason, candidates)| {
            TargetResolution::Ambiguous(TargetAlternatives { reason, candidates })
        });
    }
    let Some(tokens) = syntax_tokens_for_preproc_hit(root, offset, precedence, None, &unique_hits)
    else {
        return TargetResolution::Blocked;
    };
    TargetResolution::Resolved(TargetCandidate::new(
        SemanticTarget::Source(SourceTarget::preproc(range, tokens)),
        source_capabilities(),
    ))
}

fn test_precedence(kind: TokenKind) -> usize {
    usize::from(kind.name_like())
}
