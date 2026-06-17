use utils::line_index::TextSize;

use crate::{SyntaxNodeExt, SyntaxTree, SyntaxTreeOptions, TokenAtOffset, TokenKind};

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
    let root = tree.root().unwrap();
    let macro_start = text.find("`CA_WIDTH").unwrap();
    let offset = TextSize::from((macro_start + 1) as u32);

    let TokenAtOffset::Single(tok) = root.token_at_offset(offset) else {
        panic!("expected a token mapped to the macro invocation");
    };
    assert_eq!(tok.kind(), TokenKind::INTEGER_LITERAL);
}
