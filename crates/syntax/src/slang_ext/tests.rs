use utils::line_index::TextSize;

use crate::{
    SyntaxTree, SyntaxTreeOptions, TokenKind,
    slang_ext::{SyntaxNodeExt, TokenAtOffset},
};

#[test]
fn token_at_offset_inside_macro_invocation_does_not_descend_forever() {
    let text = r#"module ca_leaf #(
    parameter WIDTH = `CA_WIDTH,
    parameter RESET_VALUE = 0
) ();
endmodule
"#;
    let options = SyntaxTreeOptions {
        predefines: vec![String::from("CA_WIDTH=8")],
        ..SyntaxTreeOptions::default()
    };
    let tree = SyntaxTree::from_text_with_options(
        text,
        "sample/rtl/code_action_targets.v",
        "sample/rtl/code_action_targets.v",
        &options,
    );
    let root = tree.root();
    let macro_start = text.find("`CA_WIDTH").unwrap();
    let offset = TextSize::from((macro_start + 1) as u32);

    let TokenAtOffset::Single(tok) = root.token_at_offset(offset) else {
        panic!("expected a token mapped to the macro invocation");
    };
    assert_eq!(tok.kind(), TokenKind::INTEGER_LITERAL);
}

fn tree(text: &str) -> SyntaxTree {
    SyntaxTree::from_file_in_memory(text, "t.sv", "t.sv")
}

#[test]
fn plain_module_has_no_directive_trivia() {
    assert!(!tree("module m;\nendmodule\n").root().has_directive_trivia());
}

#[test]
fn define_include_ifdef_and_macro_use_have_directive_trivia() {
    assert!(tree("`define W 8\nmodule m;\nendmodule\n").root().has_directive_trivia());
    assert!(tree("`include \"a.svh\"\nmodule m;\nendmodule\n").root().has_directive_trivia());
    assert!(tree("`ifdef W\nmodule m;\nendmodule\n`endif\n").root().has_directive_trivia());
    assert!(
        tree("module m;\n  logic [`UNKNOWN-1:0] x;\nendmodule\n").root().has_directive_trivia()
    );
}
