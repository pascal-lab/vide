use base_db::{change::Change, source_root::SourceRoot};
use ide_db::root_db::RootDb;
use triomphe::Arc;
use utils::{lines::LineEnding, text_edit::TextSize};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;

fn db_with_file(text: &str) -> (RootDb, FileId, TextSize) {
    let marker = "/*caret*/";
    let offset = text.find(marker).expect("missing caret marker");
    let text = text.replace(marker, "");
    let file_id = FileId(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/test.sv".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
    });

    let mut db = RootDb::new(None);
    db.apply_change(change);
    (db, file_id, TextSize::from(offset as u32))
}

fn apply_action(text: &str, repair: RepairKind) -> Option<String> {
    let (db, file_id, offset) = db_with_file(text);
    let diagnostics = CodeActionDiagnostics { items: vec![diagnostic_for_repair(repair)] };
    let actions = code_action(
        &db,
        file_id,
        utils::text_edit::TextRange::empty(offset),
        diagnostics,
        CodeActionResolveStrategy::All,
    );
    let action = actions.into_iter().find(|action| match repair {
        RepairKind::MissingConnection => action.id.name == "add_missing_connections",
        RepairKind::MissingParameter => action.id.name == "add_missing_parameters",
        RepairKind::ConvertOrderedPorts => action.id.name == "convert_ordered_ports",
        RepairKind::ConvertOrderedParams => action.id.name == "convert_ordered_params",
        RepairKind::RemoveEmptyPortConnections => action.id.name == "remove_empty_port_connections",
        RepairKind::AddImplicitNamedPortParens => {
            action.id.name == "add_implicit_named_port_parens"
        }
        RepairKind::AddInstanceParens => action.id.name == "add_instance_parens",
    })?;
    let mut text = text.replace("/*caret*/", "");
    let edit = action.source_change?.text_edits.remove(&file_id)?;
    edit.apply(&mut text);
    Some(text)
}

fn apply_action_without_diagnostics(text: &str, action_name: &str) -> Option<String> {
    apply_action_without_diagnostics_by(text, |action| action.id.name == action_name)
}

fn apply_action_without_diagnostics_with_label(
    text: &str,
    action_name: &str,
    label: &str,
) -> Option<String> {
    apply_action_without_diagnostics_by(text, |action| {
        action.id.name == action_name && action.label == label
    })
}

fn apply_action_without_diagnostics_by(
    text: &str,
    pred: impl Fn(&CodeAction) -> bool,
) -> Option<String> {
    let (db, file_id, offset) = db_with_file(text);
    let actions = code_action(
        &db,
        file_id,
        utils::text_edit::TextRange::empty(offset),
        CodeActionDiagnostics::default(),
        CodeActionResolveStrategy::All,
    );
    let action = actions.into_iter().find(pred)?;
    let mut text = text.replace("/*caret*/", "");
    let edit = action.source_change?.text_edits.remove(&file_id)?;
    edit.apply(&mut text);
    Some(text)
}

fn diagnostic_for_repair(repair: RepairKind) -> CodeActionDiagnostic {
    match repair {
        RepairKind::MissingConnection => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("UnconnectedNamedPort".to_owned()),
            option: Some("unconnected-port".to_owned()),
        },
        RepairKind::MissingParameter => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: Some(DiagnosticCode { subsystem: 2, code: 29 }),
            name: Some("ParamHasNoValue".to_owned()),
            option: None,
        },
        RepairKind::ConvertOrderedPorts => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedPorts".to_owned()),
            option: None,
        },
        RepairKind::ConvertOrderedParams => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedParams".to_owned()),
            option: None,
        },
        RepairKind::RemoveEmptyPortConnections => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedPorts".to_owned()),
            option: None,
        },
        RepairKind::AddImplicitNamedPortParens => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("ImplicitNamedPortNotFound".to_owned()),
            option: None,
        },
        RepairKind::AddInstanceParens => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("InstanceMissingParens".to_owned()),
            option: None,
        },
    }
}

fn action_labels(text: &str, repair: RepairKind) -> Vec<String> {
    let (db, file_id, offset) = db_with_file(text);
    let diagnostics = CodeActionDiagnostics { items: vec![diagnostic_for_repair(repair)] };
    code_action(
        &db,
        file_id,
        utils::text_edit::TextRange::empty(offset),
        diagnostics,
        CodeActionResolveStrategy::None,
    )
    .into_iter()
    .map(|action| action.label)
    .collect()
}

fn action_labels_without_diagnostics(text: &str) -> Vec<String> {
    let (db, file_id, offset) = db_with_file(text);
    code_action(
        &db,
        file_id,
        utils::text_edit::TextRange::empty(offset),
        CodeActionDiagnostics::default(),
        CodeActionResolveStrategy::None,
    )
    .into_iter()
    .map(|action| action.label)
    .collect()
}

#[test]
fn remove_empty_port_connection_repair_requires_matching_diagnostic() {
    let (db, file_id, offset) = db_with_file(
        "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n",
    );
    let actions = code_action(
        &db,
        file_id,
        utils::text_edit::TextRange::empty(offset),
        CodeActionDiagnostics { items: vec![diagnostic_for_repair(RepairKind::MissingParameter)] },
        CodeActionResolveStrategy::All,
    );

    assert!(actions.iter().all(|action| action.id.name != "remove_empty_port_connections"));
}

#[test]
fn remove_empty_port_connection_requires_diagnostics() {
    let labels = action_labels_without_diagnostics(
        "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a(), ); endmodule\n",
    );

    assert!(!labels.iter().any(|label| label == "Remove empty port connections"));
}

#[test]
fn convert_ordered_ports_is_available_without_diagnostics() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/x, y); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_ordered_ports").unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
    );
}

#[test]
fn convert_ordered_params_is_available_without_diagnostics() {
    let text = "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/8, 16) u(); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_ordered_params").unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(.A(8), .B(16)) u(); endmodule\n"
    );
}

#[test]
fn literal_base_converts_plain_decimal_to_sized_signed_hexadecimal() {
    let text = "module top; localparam int value = /*caret*/42; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_label(
        text,
        "convert_literal_base",
        "Convert literal to hexadecimal",
    )
    .unwrap();

    assert_eq!(fixed, "module top; localparam int value = 32'sh2a; endmodule\n");
}

#[test]
fn literal_base_preserves_plain_decimal_sign_bit() {
    let text = "module top; localparam longint value = /*caret*/2147483648; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_label(
        text,
        "convert_literal_base",
        "Convert literal to hexadecimal",
    )
    .unwrap();

    assert_eq!(fixed, "module top; localparam longint value = 33'sh80000000; endmodule\n");
}

#[test]
fn literal_base_preserves_size_and_signed_base() {
    let text = "module top; localparam logic [7:0] value = /*caret*/8'sh2A; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_label(
        text,
        "convert_literal_base",
        "Convert literal to binary",
    )
    .unwrap();

    assert_eq!(fixed, "module top; localparam logic [7:0] value = 8'sb101010; endmodule\n");
}

#[test]
fn literal_base_converts_unsized_based_literal_to_based_decimal() {
    let text = "module top; localparam int value = /*caret*/'hff; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_label(
        text,
        "convert_literal_base",
        "Convert literal to decimal",
    )
    .unwrap();

    assert_eq!(fixed, "module top; localparam int value = 'd255; endmodule\n");
}

#[test]
fn literal_base_preserves_unsized_signed_base() {
    let text = "module top; localparam int value = /*caret*/'shff; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_label(
        text,
        "convert_literal_base",
        "Convert literal to decimal",
    )
    .unwrap();

    assert_eq!(fixed, "module top; localparam int value = 'sd255; endmodule\n");
}

#[test]
fn literal_base_does_not_offer_decimal_for_unknown_bits() {
    let labels = action_labels_without_diagnostics(
        "module top; logic [3:0] value = /*caret*/'hx; endmodule\n",
    );

    assert!(labels.iter().any(|label| label == "Convert literal to binary"));
    assert!(!labels.iter().any(|label| label == "Convert literal to decimal"));
}

#[test]
fn literal_base_is_not_available_for_string_literals() {
    let labels = action_labels_without_diagnostics(
        "module top; string value = /*caret*/\"42\"; endmodule\n",
    );

    assert!(!labels.iter().any(|label| label.starts_with("Convert literal to ")));
}

#[test]
fn missing_connection_repair_fills_named_connections() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
    );
}

#[test]
fn missing_connection_repair_is_available_without_diagnostics() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n";
    let labels = action_labels_without_diagnostics(text);
    assert!(labels.iter().any(|label| label == "Fill connections"));

    let fixed = apply_action_without_diagnostics(text, "add_missing_connections").unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
    );
}

#[test]
fn missing_connection_repair_handles_one_line_trailing_comma() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a(),); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
    );
}

#[test]
fn missing_connection_repair_preserves_multiline_named_style() {
    let text = "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    /*caret*/.a()\n);\nendmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    .a(),\n    .b(),\n    .c()\n);\nendmodule\n"
    );
}

#[test]
fn missing_connection_repair_preserves_multiline_trailing_comma_style() {
    let text = "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    /*caret*/.a(),\n);\nendmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    .a(),\n    .b(),\n    .c(),\n);\nendmodule\n"
    );
}

#[test]
fn missing_connection_repair_fills_empty_named_connection_list() {
    let text =
        "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
    );
}

#[test]
fn missing_connection_repair_fills_ordered_connections() {
    let text = "module child(input a, input b, input c); endmodule\nmodule top; logic b, c; child u(/*caret*/1'b0); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b, input c); endmodule\nmodule top; logic b, c; child u(1'b0, b, c); endmodule\n"
    );
}

#[test]
fn missing_connection_repair_uses_valid_ordered_placeholders() {
    let text = "module child(input a, input b, input c); endmodule\nmodule top; child u(/*caret*/1'b0); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b, input c); endmodule\nmodule top; child u(1'b0, /* b */ '0, /* c */ '0); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_fills_named_parameters() {
    let text = "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(.A(1), .B()) u(); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_is_available_without_diagnostics() {
    let text = "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "add_missing_parameters").unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(.A(1), .B()) u(); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_preserves_multiline_trailing_comma_style() {
    let text = "module child #(parameter A = 1, parameter B, parameter C) (); endmodule\nmodule top;\nchild #(\n    /*caret*/.A(1),\n) u();\nendmodule\n";
    let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A = 1, parameter B, parameter C) (); endmodule\nmodule top;\nchild #(\n    .A(1),\n    .B(),\n    .C(),\n) u();\nendmodule\n"
    );
}

#[test]
fn missing_parameter_repair_fills_empty_parameter_list() {
    let text = "module child #(parameter A, parameter B) (); endmodule\nmodule top; child #(/*caret*/) u(); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A, parameter B) (); endmodule\nmodule top; child #(.A(), .B()) u(); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_fills_ordered_parameters() {
    let text = "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; parameter B = 2; parameter C = 3; child #(/*caret*/1) u(); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; parameter B = 2; parameter C = 3; child #(1, B, C) u(); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_uses_valid_ordered_placeholders() {
    let text = "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; child #(/*caret*/1) u(); endmodule\n";
    let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; child #(1, /* B */ 0, /* C */ 0) u(); endmodule\n"
    );
}

#[test]
fn missing_parameter_repair_is_not_offered_when_nothing_is_missing() {
    let labels = action_labels(
        "module child #(parameter A = 1) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n",
        RepairKind::MissingParameter,
    );
    assert!(!labels.iter().any(|label| label == "Fill parameters"));
}

#[test]
fn convert_ordered_ports_repair_names_ordered_connections() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/x, .b(y)); endmodule\n";
    let fixed = apply_action(text, RepairKind::ConvertOrderedPorts).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
    );
}

#[test]
fn remove_empty_port_connection_repair_removes_trailing_comma() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y),/*caret*/); endmodule\n";
    let fixed = apply_action(text, RepairKind::RemoveEmptyPortConnections).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
    );
}

#[test]
fn remove_empty_port_connection_repair_removes_middle_empty_connection() {
    let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a(x), , .b(y)); endmodule\n";
    let fixed = apply_action(text, RepairKind::RemoveEmptyPortConnections).unwrap();
    assert_eq!(
        fixed,
        "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
    );
}

#[test]
fn convert_ordered_params_repair_names_ordered_assignments() {
    let text = "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/8, .B(16)) u(); endmodule\n";
    let fixed = apply_action(text, RepairKind::ConvertOrderedParams).unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(.A(8), .B(16)) u(); endmodule\n"
    );
}

#[test]
fn implicit_named_port_repair_adds_empty_parens() {
    let text = "module child(input a); endmodule\nmodule top; child u(/*caret*/.a); endmodule\n";
    let fixed = apply_action(text, RepairKind::AddImplicitNamedPortParens).unwrap();
    assert_eq!(fixed, "module child(input a); endmodule\nmodule top; child u(.a()); endmodule\n");
}

#[test]
fn implicit_named_port_repair_is_available_without_diagnostics() {
    let text = "module child(input a); endmodule\nmodule top; child u(/*caret*/.a); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "add_implicit_named_port_parens").unwrap();
    assert_eq!(fixed, "module child(input a); endmodule\nmodule top; child u(.a()); endmodule\n");
}

#[test]
fn instance_missing_parens_repair_adds_port_list() {
    let text = "module child; endmodule\nmodule top; child u/*caret*/; endmodule\n";
    let fixed = apply_action(text, RepairKind::AddInstanceParens).unwrap();
    assert_eq!(fixed, "module child; endmodule\nmodule top; child u(); endmodule\n");
}

#[test]
fn instance_missing_parens_repair_requires_diagnostics() {
    let text = "module child; endmodule\nmodule top; child u/*caret*/; endmodule\n";
    let labels = action_labels_without_diagnostics(text);
    assert!(!labels.iter().any(|label| label == "Add empty instance port list"));
}
