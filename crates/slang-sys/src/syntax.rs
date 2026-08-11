pub mod ast;
mod cursor;
mod element;
pub(crate) mod ffi;
mod iter;
mod syntax_kind;
mod syntax_node;
mod tree;
mod trivia;
mod walk;

pub use cursor::*;
pub use element::*;
pub use iter::*;
pub use syntax_kind::*;
pub use syntax_node::*;
pub use tree::*;
pub use trivia::*;
pub use walk::*;

#[cfg(test)]
mod tests {
    use super::{ast::AstNode, *};

    #[test]
    fn rust_calls_upstream_slang_parser() {
        let test_verilog_code = r#"
module demo(
    input wire a,
    output wire b
);
begin
    assign b = a;
end
endmodule
        "#;
        let tree = SyntaxTree::from_text_with_options(
            test_verilog_code,
            "parser_demo",
            "parser_demo.sv",
            &Default::default(),
        );
        let root = tree.root();
        assert_eq!(root.kind(), SyntaxKind::MODULE_DECLARATION);
    }

    #[test]
    fn file_parse_preserves_compilation_unit_root() {
        let tree = SyntaxTree::from_file_in_memory("module demo; endmodule", "source", "source.sv");

        assert_eq!(tree.root().kind(), SyntaxKind::COMPILATION_UNIT);
    }

    #[test]
    fn source_buffer_identity_does_not_follow_macro_expansion_root() {
        let source = "`define DECL module generated; endmodule\n`DECL\n";
        let parsed = SyntaxTree::from_file_in_memory_with_options_and_trace(
            source,
            "source",
            "source.sv",
            &Default::default(),
        );
        let trace = parsed.preprocessor_trace.expect("trace should be collected");

        assert_eq!(parsed.tree.buffer_id(), trace.root_buffer_id);
        assert_eq!(trace.source_buffers.len(), 1);
        assert_eq!(trace.source_buffers[0].buffer_id, trace.root_buffer_id);
    }

    #[test]
    fn predefine_source_buffers_use_logical_api_origin() {
        let options = SyntaxTreeOptions {
            predefines: vec!["FEATURE=1".to_owned()],
            ..SyntaxTreeOptions::default()
        };
        let parsed = SyntaxTree::from_file_in_memory_with_options_and_trace(
            "module m; wire x = `FEATURE; endmodule\n",
            "source",
            "source.sv",
            &options,
        );
        let trace = parsed.preprocessor_trace.expect("trace should be collected");
        let predefine = trace
            .source_buffers
            .iter()
            .find(|buffer| {
                buffer.text.as_deref().is_some_and(|text| text.contains("`define FEATURE"))
            })
            .expect("predefine backing buffer should be recorded");
        assert_eq!(predefine.text.as_deref(), Some("`define FEATURE 1\n"));
        assert_eq!(predefine.origin, crate::source_buffer::SourceBufferOrigin::Predefine);
    }

    #[test]
    fn syntax_tree_and_generated_accessors_work() {
        let tree = SyntaxTree::from_text_with_options(
            "module demo; endmodule",
            "accessor_demo",
            "accessor_demo.sv",
            &Default::default(),
        );
        let root = tree.root();
        let module = ast::ModuleDeclaration::cast(root).expect("expected module declaration");

        let header = module.header();
        assert_eq!(
            header.module_keyword().unwrap().kind(),
            crate::token::TokenKind::MODULE_KEYWORD
        );
        assert_eq!(header.name().unwrap().kind(), crate::token::TokenKind::IDENTIFIER);
        assert_eq!(header.name().unwrap().value_text(), "demo");

        assert_eq!(module.endmodule().unwrap().kind(), crate::token::TokenKind::END_MODULE_KEYWORD);
    }

    #[test]
    fn nested_list_accessors_stay_aligned() {
        let source = r#"
module demo(input wire a, output wire b);
    assign b = a;
endmodule
"#;
        let tree = SyntaxTree::from_text_with_options(
            source,
            "aligned_demo",
            "aligned_demo.sv",
            &Default::default(),
        );
        let module =
            ast::ModuleDeclaration::cast(tree.root()).expect("expected module declaration");

        let header = module.header();
        assert!(header.parameters().is_none());
        let ports = header.ports().expect("expected port list");
        let ansi = ports.as_ansi_port_list().expect("expected ansi port list");
        let mut ports = ansi.ports().children();

        let first = ports.next().expect("expected first port");
        let first = first.as_implicit_ansi_port().expect("expected implicit ansi port");
        let first_header = first.header().as_net_port_header().unwrap();
        assert_eq!(
            first_header.direction().unwrap().kind(),
            crate::token::TokenKind::INPUT_KEYWORD
        );
        assert_eq!(first.declarator().name().unwrap().value_text(), "a");

        let second = ports.next().expect("expected second port");
        let second = second.as_implicit_ansi_port().expect("expected implicit ansi port");
        let second_header = second.header().as_net_port_header().unwrap();
        assert_eq!(
            second_header.direction().unwrap().kind(),
            crate::token::TokenKind::OUTPUT_KEYWORD
        );
        assert_eq!(second.declarator().name().unwrap().value_text(), "b");
        assert!(ports.next().is_none());

        let mut members = module.members().children();
        let assign = members
            .next()
            .expect("expected module member")
            .as_continuous_assign()
            .expect("expected continuous assign");
        assert_eq!(assign.assign().unwrap().kind(), crate::token::TokenKind::ASSIGN_KEYWORD);
        assert_eq!(assign.assignments().children().count(), 1);
        assert_eq!(assign.semi().unwrap().kind(), crate::token::TokenKind::SEMICOLON);
        assert!(members.next().is_none());
    }

    #[test]
    fn syntax_tree_diagnostics_are_owned_rust_values() {
        let tree = SyntaxTree::from_text_with_options(
            "module A( input a; endmodule",
            "diagnostic_demo",
            "",
            &Default::default(),
        );
        let diagnostics = tree.diagnostics(&[]);

        assert!(!diagnostics.is_empty(), "expected parse diagnostics");

        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.severity, crate::diagnostic::DiagnosticSeverity::Error);
        assert!(!diagnostic.message.is_empty(), "expected formatted diagnostic message");
        assert!(diagnostic.location.is_some(), "expected diagnostic location");
    }

    #[test]
    fn syntax_tree_root_range_and_first_token_are_byte_stable() {
        let source = "module demo; endmodule";
        let tree = SyntaxTree::from_text_with_options(
            source,
            "range_demo",
            "range_demo.sv",
            &Default::default(),
        );

        let root = tree.root();
        let range = root.range().expect("expected root range");
        assert_eq!(range.start(), 0);
        assert_eq!(range.end(), source.len());
        assert_eq!(range.start_buffer_id(), range.end_buffer_id());

        let first = root.first_token().expect("expected first token");
        assert_eq!(first.kind(), crate::token::TokenKind::MODULE_KEYWORD);
        assert_eq!(first.tok.value_text(), "module");
        assert_eq!(first.range().expect("expected token range").start(), 0);
    }

    #[test]
    fn syntax_trivia_and_preorder_walk_expose_the_expected_shape() {
        let source = "// lead comment\nmodule demo; endmodule";
        let tree = SyntaxTree::from_text_with_options(
            source,
            "trivia_demo",
            "trivia_demo.sv",
            &Default::default(),
        );

        let root = tree.root();
        let first = root.first_token().expect("expected first token");
        let trivias: Vec<_> = first.trivias().collect();
        assert!(
            trivias.iter().any(|trivia| trivia.kind() == crate::token::TriviaKind::LINE_COMMENT)
        );

        let (loc, trivia) = first.trivias_with_loc().next().expect("expected trivia location");
        assert_eq!(loc.buffer_id, root.range().expect("expected root range").start_buffer_id());
        assert_eq!(loc.start, 0);
        assert!(!trivia.get_raw_text().is_empty());

        let events: Vec<_> = root.node_preorder().collect();
        assert!(matches!(events.first(), Some(WalkEvent::Enter(node)) if *node == root));
        assert!(
            events.iter().any(|event| { matches!(event, WalkEvent::Enter(node) if *node != root) })
        );
    }

    #[test]
    fn syntax_node_children_and_elements_report_parent_kind_and_range() {
        let source = "module demo; assign x = y; endmodule";
        let tree = SyntaxTree::from_text_with_options(
            source,
            "element_demo",
            "element_demo.sv",
            &Default::default(),
        );

        let root = tree.root();
        assert_eq!(root.kind(), SyntaxKind::MODULE_DECLARATION);
        assert!(root.parent().is_none());
        assert!(root.child_count() > 0);

        let first_child = root.children().next().expect("expected child element");
        assert_eq!(first_child.parent(), Some(root));
        assert!(first_child.range().expect("expected child range").is_single_buffer());
        assert!(matches!(
            first_child.kind(),
            SyntaxElementKind::Node(_) | SyntaxElementKind::Token(_)
        ));

        let first_token = root.first_token().expect("expected first token");
        let token_element = SyntaxElement::Token(first_token);
        assert!(token_element.as_node().is_none());
        assert_eq!(
            token_element.as_token().expect("expected token element").kind(),
            first_token.kind()
        );
        assert_eq!(token_element.parent(), Some(first_token.parent));
    }

    #[test]
    fn syntax_cursor_moves_between_root_and_children() {
        let source = "module demo; assign x = y; endmodule";
        let tree = SyntaxTree::from_text_with_options(
            source,
            "cursor_demo",
            "cursor_demo.sv",
            &Default::default(),
        );

        let root = tree.root();
        let mut cursor = root.walk();
        assert!(cursor.is_root());
        assert_eq!(cursor.to_node(), Some(root));

        assert!(cursor.goto_first_child());
        assert!(!cursor.is_root());
        assert_eq!(cursor.to_elem().parent(), Some(root));

        assert!(cursor.goto_parent());
        assert!(cursor.is_root());
        assert_eq!(cursor.to_node(), Some(root));

        assert!(cursor.goto_last_child());
        assert_eq!(cursor.to_elem().parent(), Some(root));
        cursor.reset_to_root();
        assert!(cursor.is_root());
    }

    #[test]
    fn syntax_element_preorder_visits_nodes_and_tokens() {
        let source = "module demo; endmodule";
        let tree = SyntaxTree::from_text_with_options(
            source,
            "walk_demo",
            "walk_demo.sv",
            &Default::default(),
        );

        let root = tree.root();
        let events: Vec<_> = root.elem_preorder().collect();

        assert!(matches!(
            events.first(),
            Some(WalkEvent::Enter(SyntaxElement::Node(node))) if *node == root
        ));
        assert!(events.iter().any(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token)) => {
                token.kind() == crate::token::TokenKind::MODULE_KEYWORD
            }
            _ => false,
        }));
        assert!(events.iter().any(|event| matches!(event, WalkEvent::Leave(_))));
    }
}
