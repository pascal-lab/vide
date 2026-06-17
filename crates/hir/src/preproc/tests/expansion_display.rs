use super::*;
use crate::hir_def::macro_file::{MacroFileExpansionDefinition, macro_file_expansion};

#[test]
fn preproc_macro_expansion_exposes_macro_file_text() {
    let root_text = r#"`define MAKE_DECL(name) logic name;
module top;
`MAKE_DECL(generated)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let macro_file = single_macro_file_at(&db, TOP, offset(root_text, "`MAKE_DECL"));
    let metadata = macro_file_expansion(&db, macro_file).expect("MAKE_DECL expansion expected");
    assert!(matches!(
        &metadata.definition,
        MacroFileExpansionDefinition::Source(definition)
            if definition.name.as_str() == "MAKE_DECL"
    ));

    let expansion = db.macro_expansion(macro_file);
    assert_eq!(expansion.text, "\nlogic generated;");
}

#[test]
fn preproc_macro_expansion_text_keeps_emitted_token_trivia() {
    let root_text = r#"`define BLOCK(name) \
  always_ff @(posedge clk) begin \
    name <= 1; \
  end
module top;
  `BLOCK(q)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let macro_file = single_macro_file_at(&db, TOP, offset(root_text, "`BLOCK"));
    let expansion = db.macro_expansion(macro_file);

    assert!(
        expansion.text.contains("\n  always_ff")
            && expansion.text.contains("\n    q <= 1;")
            && expansion.text.contains("\n  end"),
        "expansion text should preserve emitted token trivia: {:?}",
        expansion.text
    );
}

#[test]
fn preproc_maps_nested_actual_argument_macro_usage_without_dropping_expansion() {
    let root_text = r#"`define PAYL payload_i
`define NEXT(x) ((x) + 12'd1)
module top(input logic [3:0] payload_i, output logic [3:0] y);
assign y = `NEXT(`PAYL);
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let payl = macro_reference_definitions_at(&db, TOP, offset_after(root_text, "`NEXT("))
        .unwrap()
        .expect("nested actual-argument macro reference should be mapped");
    assert_eq!(text_at_range(root_text, payl.range), "`PAYL");
    assert!(
        payl.definitions
            .iter()
            .any(|definition| { definition.file_id == TOP && definition.name.as_str() == "PAYL" })
    );

    let start = offset(root_text, "`NEXT");
    let end = offset_after(root_text, "`PAYL)");
    let resolutions = macro_call_resolutions_in_range(&db, TOP, TextRange::new(start, end))
        .expect("NEXT call should resolve");
    let next = resolutions
        .iter()
        .find(|resolution| resolution.definition.name.as_str() == "NEXT")
        .expect("NEXT call should expose its written actual argument");
    let argument = next
        .call
        .arguments
        .iter()
        .find(|argument| argument.argument_index == 0)
        .expect("NEXT call should expose its first actual argument");
    assert_eq!(text_at_range(root_text, argument.range.unwrap()), "`PAYL");
    assert_eq!(
        argument.tokens.iter().map(|token| token.raw.as_str()).collect::<Vec<_>>(),
        vec!["`PAYL"]
    );

    let payl_offset = offset(root_text, "`PAYL");
    let mut names = hir_macro_files_at_offset(&db, TOP, payl_offset)
        .into_iter()
        .filter_map(|macro_file| macro_file_expansion(&db, macro_file))
        .map(|expansion| expansion_definition_name(&expansion.definition).to_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, vec!["NEXT", "PAYL"]);
}

#[test]
fn preproc_numeric_literal_expansion_text_is_available() {
    let root_text = r#"`define ONE 12'd1
module top;
localparam int W = `ONE;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let macro_file = single_macro_file_at(&db, TOP, offset(root_text, "`ONE"));
    let expansion = db.macro_expansion(macro_file);
    assert!(expansion.text.contains("12"));
    assert!(expansion.text.contains("'d"));
    assert!(expansion.text.contains("1"));
}

#[test]
fn preproc_escaped_identifier_expansion_text_is_available() {
    let root_text = concat!(
        "`define ESCAPED \\escaped.name \n",
        "module top;\n",
        "wire `ESCAPED;\n",
        "endmodule\n",
    );
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let macro_file = single_macro_file_at(&db, TOP, offset(root_text, "`ESCAPED"));
    let expansion = db.macro_expansion(macro_file);
    assert!(expansion.text.contains("\\escaped.name"));
}

fn expansion_definition_name(definition: &MacroFileExpansionDefinition) -> &str {
    match definition {
        MacroFileExpansionDefinition::Source(definition) => definition.name.as_str(),
        MacroFileExpansionDefinition::Builtin { name } => name.as_str(),
    }
}
