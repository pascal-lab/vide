pub mod ast;
mod ffi;
mod syntax_kind;
mod syntax_node;

pub use syntax_kind::*;
pub use syntax_node::*;

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
        let tree = SyntaxTree::from_text(test_verilog_code, "parser_demo", "parser_demo.sv");
        let root = tree.root().expect("expected syntax root");
        assert_eq!(root.kind(), SyntaxKind::MODULE_DECLARATION);
    }

    #[test]
    fn syntax_tree_and_generated_accessors_work() {
        let tree =
            SyntaxTree::from_text("module demo; endmodule", "accessor_demo", "accessor_demo.sv");
        let root = tree.root().expect("expected syntax root");
        let module = ast::ModuleDeclaration::cast(root).expect("expected module declaration");

        let header = module.header().expect("expected module header");
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
        let tree = SyntaxTree::from_text(source, "aligned_demo", "aligned_demo.sv");
        let module = ast::ModuleDeclaration::cast(tree.root().expect("expected root"))
            .expect("expected module declaration");

        let header = module.header().expect("expected module header");
        assert!(header.parameters().is_none());
        let ports = header.ports().expect("expected port list");
        let ansi = ports.as_ansi_port_list().expect("expected ansi port list");
        let mut ports = ansi.ports().children();

        let first = ports.next().expect("expected first port");
        let first = first.as_implicit_ansi_port().expect("expected implicit ansi port");
        let first_header = first.header().unwrap().as_net_port_header().unwrap();
        assert_eq!(
            first_header.direction().unwrap().kind(),
            crate::token::TokenKind::INPUT_KEYWORD
        );
        assert_eq!(first.declarator().unwrap().name().unwrap().value_text(), "a");

        let second = ports.next().expect("expected second port");
        let second = second.as_implicit_ansi_port().expect("expected implicit ansi port");
        let second_header = second.header().unwrap().as_net_port_header().unwrap();
        assert_eq!(
            second_header.direction().unwrap().kind(),
            crate::token::TokenKind::OUTPUT_KEYWORD
        );
        assert_eq!(second.declarator().unwrap().name().unwrap().value_text(), "b");
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
}
