use super::*;
use crate::hir_def::macro_file::{MacroExpansionDefinition, macro_file_expansion};

#[test]
fn preproc_include_usage_resolves_to_header_define() {
    let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
    let header_text = "`define HEADER_WIDTH 8\n";
    let db = db_with_files(root_text, header_text);

    let resolution =
        macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH")).unwrap().unwrap();
    assert_eq!(resolution.usage.file_id, TOP);
    assert_eq!(resolution.definition.file_id, HEADER);
    assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
    assert_eq!(text_at_range(header_text, resolution.definition.name_range), "HEADER_WIDTH");

    let include = include_directive_at(&db, TOP, offset(root_text, "defs.vh")).unwrap().unwrap();
    assert_eq!(text_at_range(root_text, include.range), "\"defs.vh\"");
    assert!(include_directive_at(&db, TOP, offset(root_text, "`include")).unwrap().is_none());
    assert!(include_directive_at(&db, TOP, include.range.end()).unwrap().is_none());
    let IncludeTarget::Literal { resolved_file, .. } = include.target else {
        panic!("literal include expected");
    };
    assert_eq!(resolved_file, Some(HEADER));
}

#[test]
fn preproc_macro_expansion_queries_map_call_ranges() {
    let root_text = r#"`define OBJ 8
`define LEAF 3
`define WRAP `LEAF
module top;
localparam int A = `OBJ;
localparam int B = `WRAP;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let obj_file = single_macro_file_at(&db, TOP, offset(root_text, "`OBJ"));
    let obj = macro_file_expansion(&db, obj_file).expect("object-like macro expansion expected");
    assert_eq!(obj.call_file_id, TOP);
    assert_eq!(text_at_range(root_text, obj.call_range), "`OBJ");
    assert_eq!(db.macro_expansion(obj_file).value.text.trim(), "8");

    let wrap_file = single_macro_file_at(&db, TOP, offset(root_text, "`WRAP"));
    let wrap = macro_file_expansion(&db, wrap_file).expect("outer macro expansion expected");
    assert_eq!(wrap.call_file_id, TOP);
    assert_eq!(text_at_range(root_text, wrap.call_range), "`WRAP");
    assert!(matches!(
        &wrap.definition,
        MacroExpansionDefinition::Source(definition) if definition.name.as_str() == "WRAP"
    ));
    assert_eq!(db.macro_expansion(wrap_file).value.text.trim(), "3");
}

#[test]
fn preproc_macro_call_resolutions_in_range_map_formal_params() {
    let root_text = "\
`define MAKE(width, expr) logic [width-1:0] expr
module top; `MAKE(8, data_q) endmodule
";
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);
    let start = offset(root_text, "`MAKE");
    let end = offset_after(root_text, "data_q");

    let resolutions =
        macro_call_resolutions_in_range(&db, TOP, TextRange::new(start, end)).unwrap();

    assert_eq!(resolutions.len(), 1);
    let resolution = &resolutions[0];
    assert_eq!(text_at_range(root_text, resolution.call.range), "`MAKE(8, data_q)");
    assert_eq!(
        resolution
            .definition
            .params
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|param| param.name.as_deref())
            .collect::<Vec<_>>(),
        vec!["width", "expr"]
    );
    assert_eq!(
        resolution
            .call
            .arguments
            .iter()
            .filter_map(|argument| argument.range.map(|range| text_at_range(root_text, range)))
            .collect::<Vec<_>>(),
        vec!["8", "data_q"]
    );
}

#[test]
fn preproc_builtin_intrinsic_expansion_uses_structured_diagnostic_target() {
    let root_text = r#"module m;
localparam int L = `__LINE__;
localparam string F = `__FILE__;
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    let line_offset = offset(root_text, "`__LINE__");
    let file_offset = offset(root_text, "`__FILE__");
    for (offset, expected_name) in [(line_offset, "__LINE__"), (file_offset, "__FILE__")] {
        let macro_file = single_macro_file_at(&db, TOP, offset);
        let expansion = macro_file_expansion(&db, macro_file).expect("builtin call expected");
        assert!(matches!(
            expansion.definition,
            MacroExpansionDefinition::Builtin { name, .. } if name.as_str() == expected_name
        ));

        let diagnostic = diagnostic_target_for_range(&db, TOP, expansion.call_range)
            .unwrap()
            .target
            .expect("diagnostic target expected");
        assert!(matches!(
            diagnostic.origin,
            crate::hir_def::macro_file::Origin::Builtin { name, .. }
                if name.as_str() == expected_name
        ));
        assert_eq!(diagnostic.file_id, TOP);
        assert_eq!(diagnostic.range, expansion.call_range);
    }
}

#[test]
fn preproc_zero_token_macro_expansion_is_available() {
    let root_text = r#"`define EMPTY
`define DROP(x)
module top;
`EMPTY
`DROP(foo)
endmodule
"#;
    let db = db_with_entries(&[(TOP, "rtl/top.v", root_text)]);

    for (name, call_text) in [("`EMPTY", "`EMPTY"), ("`DROP", "`DROP(foo)")] {
        let macro_file = single_macro_file_at(&db, TOP, offset(root_text, name));
        let expansion = macro_file_expansion(&db, macro_file).expect("macro expansion expected");
        assert_eq!(text_at_range(root_text, expansion.call_range), call_text);
        let result = db.macro_expansion(macro_file);
        assert_eq!(result.value.text, "");
        assert_eq!(result.err, None);
    }
}
