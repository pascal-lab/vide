use source_model::{
    FilePosition as SourceFilePosition, MacroArgumentTokenIdentity, MacroBodyTokenIdentity,
    MacroCallIdentity, MacroDefinitionIdentity, MacroExpansionIdentity,
    MacroOperationTokenIdentity, SourceOrigin, SourcePurpose, SourceRangeResult, SpanId,
};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, WalkEvent,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use super::source_graph_model_file_ids_for_file;
use crate::base_db::source_db::SourceRootDb;

#[derive(Debug, Clone)]
pub struct SyntaxTarget<'tree> {
    range: TextRange,
    tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SyntaxTarget<'tree> {
    pub fn into_tokens(self) -> Vec<SyntaxTokenWithParent<'tree>> {
        self.tokens
    }

    pub fn into_parts(self) -> (TextRange, Vec<SyntaxTokenWithParent<'tree>>) {
        (self.range, self.tokens)
    }
}

pub fn syntax_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: impl Fn(TokenKind) -> usize,
) -> Option<SyntaxTarget<'tree>> {
    let token = root.token_at_offset(offset).pick_bext_token(precedence)?;
    let range = token.text_range()?;
    Some(SyntaxTarget { range, tokens: vec![token] })
}

pub fn left_biased_syntax_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
) -> Option<SyntaxTarget<'tree>> {
    let token = root.token_at_offset(offset).left_biased()?;
    let range = token.text_range()?;
    Some(SyntaxTarget { range, tokens: vec![token] })
}

pub fn generated_syntax_target_at_offset<'tree>(
    db: &dyn SourceRootDb,
    file_id: FileId,
    root: SyntaxNode<'tree>,
    offset: TextSize,
    purpose: SourcePurpose,
) -> Option<SyntaxTarget<'tree>> {
    let mut source_ranges = Vec::new();
    let mut identities = Vec::new();

    for model_file_id in source_graph_model_file_ids_for_file(db, file_id) {
        let source_graph = db.source_graph_preproc_model(model_file_id);
        let Ok(source_graph) = source_graph.as_ref() else {
            continue;
        };
        let graph = &source_graph.graph;
        for (source_span, generated_span, _) in
            graph.generated_spelling_hits_for_file_position(SourceFilePosition { file_id, offset })
        {
            if let SourceRangeResult::Mapped(source_range) =
                graph.to_file_range(source_span, purpose)
                && source_range.file_id == file_id
            {
                source_ranges.push(source_range.range);
            }
            for identity in generated_syntax_identities_for_span(graph, generated_span) {
                if !identities.contains(&identity) {
                    identities.push(identity);
                }
            }
        }
    }

    let tokens = syntax_tokens_for_generated_identities(root, &identities)?;
    let range = covering_range(&source_ranges).unwrap_or_else(|| TextRange::empty(offset));
    Some(SyntaxTarget { range, tokens })
}

fn generated_syntax_identities_for_span(
    graph: &source_model::SourceGraph,
    span: SpanId,
) -> Vec<GeneratedSyntaxIdentity> {
    graph
        .origins_for_span(span)
        .into_iter()
        .filter_map(|origin| generated_syntax_identity_from_origin(graph.origin(origin)))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GeneratedSyntaxIdentity {
    Body(MacroBodyTokenIdentity),
    Argument(MacroArgumentTokenIdentity),
    Operation(MacroOperationTokenIdentity),
}

fn generated_syntax_identity_from_origin(origin: &SourceOrigin) -> Option<GeneratedSyntaxIdentity> {
    match origin {
        SourceOrigin::MacroBody { identity, .. } => Some(GeneratedSyntaxIdentity::Body(*identity)),
        SourceOrigin::MacroArgument { identity, .. } => {
            Some(GeneratedSyntaxIdentity::Argument(*identity))
        }
        SourceOrigin::TokenPaste { identity, .. }
        | SourceOrigin::Stringification { identity, .. } => {
            Some(GeneratedSyntaxIdentity::Operation(*identity))
        }
        SourceOrigin::Written { .. }
        | SourceOrigin::Builtin { .. }
        | SourceOrigin::Synthetic { .. }
        | SourceOrigin::Composite { .. }
        | SourceOrigin::Unavailable { .. }
        | SourceOrigin::Alias { .. } => None,
    }
}

fn syntax_tokens_for_generated_identities<'tree>(
    root: SyntaxNode<'tree>,
    identities: &[GeneratedSyntaxIdentity],
) -> Option<Vec<SyntaxTokenWithParent<'tree>>> {
    if identities.is_empty() {
        return None;
    }

    let mut tokens = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };
        let Some(identity) =
            generated_syntax_identity_from_provenance(token.preprocessor_trace_provenance())
        else {
            continue;
        };
        if identities.contains(&identity) && !tokens.contains(&token) {
            tokens.push(token);
        }
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn generated_syntax_identity_from_provenance(
    provenance: syntax::PreprocessorTraceTokenProvenance,
) -> Option<GeneratedSyntaxIdentity> {
    match provenance {
        syntax::PreprocessorTraceTokenProvenance::MacroBody { identity, .. } => {
            Some(GeneratedSyntaxIdentity::Body(MacroBodyTokenIdentity {
                call: MacroCallIdentity::new(identity.call_id.0),
                definition: MacroDefinitionIdentity::new(identity.definition_id.0),
                expansion: MacroExpansionIdentity::new(identity.expansion_id.0),
                parent_expansion: identity
                    .parent_expansion_id
                    .map(|id| MacroExpansionIdentity::new(id.0)),
                body_token_index: identity.body_token_index as usize,
            }))
        }
        syntax::PreprocessorTraceTokenProvenance::MacroArgument { identity, .. } => {
            Some(GeneratedSyntaxIdentity::Argument(MacroArgumentTokenIdentity {
                call: MacroCallIdentity::new(identity.call_id.0),
                definition: MacroDefinitionIdentity::new(identity.definition_id.0),
                expansion: MacroExpansionIdentity::new(identity.expansion_id.0),
                parent_expansion: identity
                    .parent_expansion_id
                    .map(|id| MacroExpansionIdentity::new(id.0)),
                body_token_index: identity.body_token_index as usize,
                argument_index: identity.argument_index as usize,
                argument_token_index: identity.argument_token_index as usize,
            }))
        }
        syntax::PreprocessorTraceTokenProvenance::TokenPaste { identity, .. }
        | syntax::PreprocessorTraceTokenProvenance::Stringification { identity, .. } => {
            Some(GeneratedSyntaxIdentity::Operation(MacroOperationTokenIdentity {
                call: MacroCallIdentity::new(identity.call_id.0),
                definition: MacroDefinitionIdentity::new(identity.definition_id.0),
                expansion: MacroExpansionIdentity::new(identity.expansion_id.0),
                parent_expansion: identity
                    .parent_expansion_id
                    .map(|id| MacroExpansionIdentity::new(id.0)),
                body_token_index: identity.body_token_index as usize,
                argument_index: identity.argument_index.map(|index| index as usize),
                argument_token_index: identity.argument_token_index.map(|index| index as usize),
            }))
        }
        syntax::PreprocessorTraceTokenProvenance::Source { .. }
        | syntax::PreprocessorTraceTokenProvenance::Builtin { .. }
        | syntax::PreprocessorTraceTokenProvenance::Unavailable => None,
    }
}

fn covering_range(ranges: &[TextRange]) -> Option<TextRange> {
    let start = ranges.iter().map(|range| range.start()).min()?;
    let end = ranges.iter().map(|range| range.end()).max()?;
    Some(TextRange::new(start, end))
}

#[cfg(test)]
mod tests {
    use syntax::{PreprocessorTraceTokenProvenance, SyntaxTree, SyntaxTreeOptions};

    use super::*;

    #[test]
    fn syntax_target_at_offset_returns_selected_token_range() {
        let source = "module m; wire payload_i; endmodule\n";
        let tree = SyntaxTree::from_text(source, "test", "test.sv");
        let root = tree.root().expect("test source should parse");
        let range = source_range(source, "payload_i");

        let target =
            syntax_target_at_offset(root, range.start(), test_precedence).expect("target token");
        let (target_range, tokens) = target.into_parts();

        assert_eq!(target_range, range);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    }

    #[test]
    fn left_biased_syntax_target_keeps_token_at_end_boundary() {
        let source = "module m; wire payload_i; endmodule\n";
        let tree = SyntaxTree::from_text(source, "test", "test.sv");
        let root = tree.root().expect("test source should parse");
        let range = source_range(source, "payload_i");

        let target = left_biased_syntax_target_at_offset(root, range.end()).expect("target token");
        let (target_range, tokens) = target.into_parts();

        assert_eq!(target_range, range);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    }

    #[test]
    fn generated_identity_selects_macro_argument_syntax_token() {
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
        let identity = root
            .elem_preorder()
            .find_map(|event| {
                let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
                    return None;
                };
                if token.raw_text().as_bytes() != b"payload_i" {
                    return None;
                }
                let provenance = token.preprocessor_trace_provenance();
                matches!(provenance, PreprocessorTraceTokenProvenance::MacroArgument { .. })
                    .then(|| generated_syntax_identity_from_provenance(provenance))
                    .flatten()
            })
            .expect("expanded source should contain macro argument provenance");

        let tokens = syntax_tokens_for_generated_identities(root, &[identity])
            .expect("macro argument identity should resolve to a syntax token");

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].raw_text().as_bytes(), b"payload_i");
    }

    fn source_range(text: &str, needle: &str) -> TextRange {
        let start = text.find(needle).expect("needle should exist");
        TextRange::new(TextSize::from(start as u32), TextSize::from((start + needle.len()) as u32))
    }

    fn test_precedence(_kind: TokenKind) -> usize {
        0
    }
}
