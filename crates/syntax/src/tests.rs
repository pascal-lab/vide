#[cfg(test)]
mod test {
    use crate::*;
    use ast::{self, AstNode};
    use tree_sitter::Parser;
    use tree_sitter_verilog;

    #[test]
    fn test_ast_src() {
        let mut parser = Parser::new();
        let language = tree_sitter_verilog::language();
        parser.set_language(language).expect("Error loading SV grammar");
        let source_code = r#"
module A();
wire a, b;
assign a = b + 1;
endmodule
"#;
        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();
        let source = ast::SourceFile::cast(root_node).unwrap();
        let description = source.descriptions().next().unwrap();
        let module_declaration = description.module_declaration().unwrap();
        let module_nonansi_header = module_declaration.module_nonansi_header().unwrap();
        let identifier = module_nonansi_header.identifier().unwrap();
        assert_eq!(identifier.syntax().byte_range(), std::ops::Range { start: 8, end: 9 });
    }

    #[test]
    fn test_ptr() {
        let mut parser = Parser::new();
        let language = tree_sitter_verilog::language();
        parser.set_language(language).expect("Error loading SV grammar");
        let source_code = r#"
module A();
wire a, b;
assign a = b + 1;
endmodule
"#;
        let tree = parser.parse(source_code, None).unwrap();
        let root_node = tree.root_node();
        let source = ast::SourceFile::cast(root_node).unwrap();
        let description = source.descriptions().next().unwrap();
        let module_declaration = description.module_declaration().unwrap();
        assert_eq!(description.syntax().byte_range(), std::ops::Range { start: 1, end: 51 });
        assert_eq!(module_declaration.syntax().byte_range(), std::ops::Range { start: 1, end: 51 });
        let description_ptr = description.to_ptr();
        let module_declaration_ptr = module_declaration.to_ptr();
        assert_eq!(
            SyntaxAncestors::new(&module_declaration.syntax()).next().as_ref().unwrap(),
            description.syntax()
        );
        assert!(description_ptr.to_node(&tree).is_some());
        assert!(module_declaration_ptr.to_node(&tree).is_some());
    }
}
