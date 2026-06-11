use std::{fs, path::Path};

use hir::base_db::{change::Change, source_root::SourceRoot};
use triomphe::Arc;
use utils::{
    lines::LineEnding,
    text_edit::{TextRange, TextSize},
};
use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

use super::*;
use crate::db::root_db::RootDb;

struct CodeActionFixture {
    action: FixtureAction,
    source: String,
}

enum FixtureAction {
    Action { name: String, label: Option<String> },
    Repair(RepairKind),
}

impl CodeActionFixture {
    fn read(path: &Path) -> Self {
        let raw = fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
        let mut action = None;
        let mut label = None;
        let mut repair = None;
        let mut source = String::new();

        for line in raw.lines() {
            let Some(meta) = line.strip_prefix("//- ") else {
                source.push_str(line);
                source.push('\n');
                continue;
            };

            let (key, value) = meta
                .split_once(':')
                .unwrap_or_else(|| panic!("invalid fixture metadata in {}", path.display()));
            match key.trim() {
                "action" => action = Some(value.trim().to_owned()),
                "label" => label = Some(value.trim().to_owned()),
                "repair" => repair = Some(parse_fixture_repair(value.trim(), path)),
                other => panic!("unknown fixture metadata key `{other}` in {}", path.display()),
            }
        }

        let action = match (action, repair) {
            (Some(name), None) => FixtureAction::Action { name, label },
            (None, Some(repair)) => {
                if label.is_some() {
                    panic!("repair fixture {} cannot specify label", path.display());
                }
                FixtureAction::Repair(repair)
            }
            (Some(_), Some(_)) => {
                panic!("fixture {} must specify only one of action or repair", path.display())
            }
            (None, None) => {
                panic!("fixture {} must specify one of action or repair", path.display())
            }
        };

        Self { action, source }
    }

    fn apply(&self, path: &Path) -> String {
        match &self.action {
            FixtureAction::Action { name, label } => match label {
                Some(label) => {
                    apply_action_without_diagnostics_with_label(&self.source, name, label)
                }
                None => apply_action_without_diagnostics(&self.source, name),
            },
            FixtureAction::Repair(repair) => apply_action(&self.source, *repair),
        }
        .unwrap_or_else(|| panic!("fixture {} did not produce an edit", path.display()))
    }
}

fn parse_fixture_repair(value: &str, path: &Path) -> RepairKind {
    match value {
        "MissingConnection" => RepairKind::MissingConnection,
        "MissingParameter" => RepairKind::MissingParameter,
        "ConvertOrderedPorts" => RepairKind::ConvertOrderedPorts,
        "ConvertOrderedParams" => RepairKind::ConvertOrderedParams,
        "RemoveEmptyPortConnections" => RepairKind::RemoveEmptyPortConnections,
        "AddImplicitNamedPortParens" => RepairKind::AddImplicitNamedPortParens,
        "AddInstanceParens" => RepairKind::AddInstanceParens,
        "InsertExpectedToken" => RepairKind::InsertExpectedToken,
        other => panic!("unknown fixture repair kind `{other}` in {}", path.display()),
    }
}

fn db_with_file(text: &str) -> (RootDb, FileId, TextSize) {
    let marker = "/*caret*/";
    let offset = text.find(marker).expect("missing caret marker");
    let text = text.replace(marker, "");
    let (db, file_id) = db_with_text(&text);
    (db, file_id, TextSize::from(offset as u32))
}

fn db_with_text(text: &str) -> (RootDb, FileId) {
    let file_id = FileId(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/test.sv".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.add_changed_file(ChangedFile {
        file_id,
        change_kind: ChangeKind::Create(Arc::from(text), LineEnding::Unix),
    });

    let mut db = RootDb::new(None);
    db.apply_change(change);
    (db, file_id)
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
        RepairKind::InsertExpectedToken => action.id.name == "insert_expected_token",
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

fn apply_action_without_diagnostics_with_selection(
    text: &str,
    action_name: &str,
) -> Option<String> {
    apply_action_without_diagnostics_with_selection_by(text, |action| action.id.name == action_name)
}

fn apply_action_without_diagnostics_with_selection_by(
    text: &str,
    pred: impl Fn(&CodeAction) -> bool,
) -> Option<String> {
    let (mut text, range) = text_with_selection_range(text);
    let (db, file_id) = db_with_text(&text);
    let actions = code_action(
        &db,
        file_id,
        range,
        CodeActionDiagnostics::default(),
        CodeActionResolveStrategy::All,
    );
    let action = actions.into_iter().find(pred)?;
    let edit = action.source_change?.text_edits.remove(&file_id)?;
    edit.apply(&mut text);
    Some(text)
}

fn action_labels_without_diagnostics_with_selection(text: &str) -> Vec<String> {
    let (text, range) = text_with_selection_range(text);
    let (db, file_id) = db_with_text(&text);
    code_action(
        &db,
        file_id,
        range,
        CodeActionDiagnostics::default(),
        CodeActionResolveStrategy::All,
    )
    .into_iter()
    .map(|action| action.label)
    .collect()
}

fn text_with_selection_range(text: &str) -> (String, TextRange) {
    let marker = "/*selection*/";
    let start = text.find(marker).expect("missing selection start marker");
    let text = text.replacen(marker, "", 1);
    let end = text.find(marker).expect("missing selection end marker");
    let text = text.replacen(marker, "", 1);
    let range = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    (text, range)
}

fn diagnostic_for_repair(repair: RepairKind) -> CodeActionDiagnostic {
    match repair {
        RepairKind::MissingConnection => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("UnconnectedNamedPort".to_owned()),
            option: Some("unconnected-port".to_owned()),
            range: None,
            expected_token: None,
        },
        RepairKind::MissingParameter => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: Some(DiagnosticCode { subsystem: 2, code: 29 }),
            name: Some("ParamHasNoValue".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::ConvertOrderedPorts => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedPorts".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::ConvertOrderedParams => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedParams".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::RemoveEmptyPortConnections => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("MixingOrderedAndNamedPorts".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::AddImplicitNamedPortParens => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("ImplicitNamedPortNotFound".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::AddInstanceParens => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Semantic),
            code: None,
            name: Some("InstanceMissingParens".to_owned()),
            option: None,
            range: None,
            expected_token: None,
        },
        RepairKind::InsertExpectedToken => CodeActionDiagnostic {
            source: Some(DiagnosticSource::Parse),
            code: None,
            name: Some("ExpectedToken".to_owned()),
            option: None,
            range: None,
            expected_token: Some(";".to_owned()),
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
fn code_action_edit_fixtures() {
    insta::glob!("fixtures/code_actions/*.sv", |path| {
        let fixture = CodeActionFixture::read(path);
        let fixed = fixture.apply(path);
        insta::assert_snapshot!(fixed);
    });
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
fn reformat_number_literal_requires_enough_digits() {
    let labels = action_labels_without_diagnostics(
        "module top; localparam int value = /*caret*/999; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label.starts_with("Convert 999 to ")));
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
fn named_port_shorthand_expands() {
    let text =
        "module child(input a); endmodule\nmodule top; logic a; child u(/*caret*/.a); endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "expand_named_port_connection_shorthand").unwrap();
    assert_eq!(
        fixed,
        "module child(input a); endmodule\nmodule top; logic a; child u(.a(a)); endmodule\n"
    );
}

#[test]
fn named_port_shorthand_expands_all_named_connections_in_instance() {
    let text = "module child(input a, b); endmodule\nmodule top; logic a, b; child u(/*caret*/.a, .b); endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "expand_named_port_connection_shorthand").unwrap();
    assert_eq!(
        fixed,
        "module child(input a, b); endmodule\nmodule top; logic a, b; child u(.a(a), .b(b)); endmodule\n"
    );
}

#[test]
fn named_port_shorthand_collapses() {
    let text = "module child(input a); endmodule\nmodule top; logic a; child u(/*caret*/.a(a)); endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "collapse_named_port_connection_shorthand").unwrap();
    assert_eq!(
        fixed,
        "module child(input a); endmodule\nmodule top; logic a; child u(.a); endmodule\n"
    );
}

#[test]
fn named_port_shorthand_collapses_all_named_connections_in_instance() {
    let text = "module child(input a, b); endmodule\nmodule top; logic a, b; child u(/*caret*/.a(a), .b(b)); endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "collapse_named_port_connection_shorthand").unwrap();
    assert_eq!(
        fixed,
        "module child(input a, b); endmodule\nmodule top; logic a, b; child u(.a, .b); endmodule\n"
    );
}

#[test]
fn named_port_shorthand_collapses_matching_connections_in_instance() {
    let text = "module child(input a, b, c); endmodule\nmodule top; logic sw1, b, gate_out; child u(/*caret*/.a(sw1), .c(c), .b(gate_out)); endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "collapse_named_port_connection_shorthand").unwrap();
    assert_eq!(
        fixed,
        "module child(input a, b, c); endmodule\nmodule top; logic sw1, b, gate_out; child u(.a(sw1), .c, .b(gate_out)); endmodule\n"
    );
}

#[test]
fn named_port_shorthand_collapse_requires_at_least_one_same_name() {
    let labels = action_labels_without_diagnostics(
        "module child(input a); endmodule\nmodule top; logic b; child u(/*caret*/.a(b)); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Collapse named port to shorthand"));
}

#[test]
fn named_port_shorthand_requires_all_connections_named() {
    let labels = action_labels_without_diagnostics(
        "module child(input a, b); endmodule\nmodule top; logic a, b; child u(/*caret*/.a, b); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Expand named port shorthand"));
}

#[test]
fn convert_always_star_to_always_comb() {
    let text = "module top; logic a, y; /*caret*/always @(*) begin y = a; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_always_to_always_comb").unwrap();
    assert_eq!(fixed, "module top; logic a, y; always_comb begin y = a; end endmodule\n");
}

#[test]
fn convert_always_comb_to_always_star() {
    let text = "module top; logic a, y; /*caret*/always_comb begin y = a; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_always_comb_to_always").unwrap();
    assert_eq!(fixed, "module top; logic a, y; always @(*) begin y = a; end endmodule\n");
}

#[test]
fn convert_always_posedge_to_always_ff() {
    let text = "module top; logic clk, d, q; /*caret*/always @(posedge clk) q <= d; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_always_to_always_ff").unwrap();
    assert_eq!(fixed, "module top; logic clk, d, q; always_ff @(posedge clk) q <= d; endmodule\n");
}

#[test]
fn convert_always_event_list_to_always_ff() {
    let text = "module top; logic clk, d, q; always @(/*caret*/posedge clk) q <= d; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_always_to_always_ff").unwrap();
    assert_eq!(fixed, "module top; logic clk, d, q; always_ff @(posedge clk) q <= d; endmodule\n");
}

#[test]
fn convert_always_ff_to_plain_always() {
    let text = "module top; logic clk, d, q; /*caret*/always_ff @(posedge clk) q <= d; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_always_ff_to_always").unwrap();
    assert_eq!(fixed, "module top; logic clk, d, q; always @(posedge clk) q <= d; endmodule\n");
}

#[test]
fn convert_always_block_requires_caret_on_keyword_or_event_list() {
    let labels = action_labels_without_diagnostics(
        "module top; logic a, y; always @(*) begin /*caret*/y = a; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert to always_comb"));

    let labels = action_labels_without_diagnostics(
        "module top; logic a, y; always_comb begin /*caret*/y = a; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert to always @(*)"));

    let labels = action_labels_without_diagnostics(
        "module top; logic clk, d, q; always_ff @(posedge clk) /*caret*/q <= d; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert to always @(...)"));
}

#[test]
fn convert_always_to_always_ff_requires_edge_sensitivity() {
    let labels = action_labels_without_diagnostics(
        "module top; logic clk, d, q; /*caret*/always @(clk) q <= d; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert to always_ff"));
}

#[test]
fn instance_missing_parens_repair_requires_diagnostics() {
    let text = "module child; endmodule\nmodule top; child u/*caret*/; endmodule\n";
    let labels = action_labels_without_diagnostics(text);
    assert!(!labels.iter().any(|label| label == "Add empty instance port list"));
}

#[test]
fn convert_ansi_ports_to_non_ansi() {
    let text = "module top(/*caret*/input a, output logic b);\nassign b = a;\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_ansi_ports_to_non_ansi").unwrap();
    assert_eq!(
        fixed,
        "module top(a, b);\n    input a;\n    output logic b;\n    assign b = a;\nendmodule\n"
    );
}

#[test]
fn convert_ansi_ports_to_non_ansi_uses_inherited_header() {
    let text = "module top(/*caret*/input a, b);\nassign b = a;\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_ansi_ports_to_non_ansi").unwrap();
    assert_eq!(
        fixed,
        "module top(a, b);\n    input a;\n    input wire logic b;\n    assign b = a;\nendmodule\n"
    );
}

#[test]
fn convert_non_ansi_ports_to_ansi() {
    let text =
        "module top(/*caret*/a, b);\ninput wire a;\noutput logic b;\nassign b = a;\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_non_ansi_ports_to_ansi").unwrap();
    assert_eq!(fixed, "module top(input wire a, output logic b);\n    assign b = a;\nendmodule\n");
}

#[test]
fn convert_non_ansi_ports_to_ansi_merges_data_declaration() {
    let text = "module top (\n    /*caret*/c,\n    led0\n);\n    input  wire c;\n    output led0;\n    reg led0;\n\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_non_ansi_ports_to_ansi").unwrap();
    assert_eq!(fixed, "module top (\n    input  wire c,\n    output reg led0\n);\nendmodule\n");
}

#[test]
fn convert_ansi_ports_to_non_ansi_preserves_body_comments() {
    let text =
        "module top(/*caret*/input a, output logic b);\n// keep this\nassign b = a;\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_ansi_ports_to_non_ansi").unwrap();
    assert!(fixed.contains("// keep this"), "{fixed}");
    assert!(fixed.contains("assign b = a;"), "{fixed}");
}

#[test]
fn convert_non_ansi_ports_to_ansi_preserves_body_comments() {
    let text = "module top(/*caret*/a, b);\n// keep first\ninput wire a;\n// keep second\noutput logic b;\nassign b = a;\nendmodule\n";
    let fixed = apply_action_without_diagnostics(text, "convert_non_ansi_ports_to_ansi").unwrap();
    assert!(fixed.contains("// keep first"), "{fixed}");
    assert!(fixed.contains("// keep second"), "{fixed}");
    assert!(fixed.contains("assign b = a;"), "{fixed}");
}

#[test]
fn convert_port_declarations_requires_caret_in_port_list() {
    let labels = action_labels_without_diagnostics(
        "module /*caret*/top(input a, output logic b);\nassign b = a;\nendmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert ANSI port declarations to non-ANSI"));

    let labels = action_labels_without_diagnostics(
        "module top(input a, output logic b);\n/*caret*/assign b = a;\nendmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert ANSI port declarations to non-ANSI"));

    let labels = action_labels_without_diagnostics(
        "module /*caret*/top(a, b);\ninput wire a;\noutput logic b;\nassign b = a;\nendmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert non-ANSI port declarations to ANSI"));

    let labels = action_labels_without_diagnostics(
        "module top(a, b);\ninput wire a;\n/*caret*/output logic b;\nassign b = a;\nendmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert non-ANSI port declarations to ANSI"));
}

#[test]
fn split_declaration_declarators_splits_data_declaration() {
    let text = "module top; /*caret*/logic [3:0] a, b = 4'h0; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "split_declaration_declarators").unwrap();
    assert_eq!(fixed, "module top; logic [3:0] a;\nlogic [3:0] b = 4'h0; endmodule\n");
}

#[test]
fn split_declaration_declarators_requires_multiple_declarators() {
    let labels = action_labels_without_diagnostics("module top; /*caret*/logic a; endmodule\n");
    assert!(!labels.iter().any(|label| label == "Split declaration"));
}

#[test]
fn sort_named_parameter_assignments_sorts_named_assignments() {
    let text = "module child #(parameter WIDTH = 8, parameter DEPTH = 16) (); endmodule\nmodule top; child #(/*caret*/.DEPTH(16), .WIDTH(8)) u(); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "sort_named_parameter_assignments").unwrap();
    assert_eq!(
        fixed,
        "module child #(parameter WIDTH = 8, parameter DEPTH = 16) (); endmodule\nmodule top; child #(.WIDTH(8), .DEPTH(16)) u(); endmodule\n"
    );
}

#[test]
fn sort_named_parameter_assignments_rejects_mixed_assignments() {
    let labels = action_labels_without_diagnostics(
        "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/.B(2), 1) u(); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Sort named parameter assignments"));
}

#[test]
fn sort_named_port_connections_sorts_named_connections() {
    let text = "module child(input z, input a); endmodule\nmodule top; child u(/*caret*/.a(y), .z(x)); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "sort_named_port_connections").unwrap();
    assert_eq!(
        fixed,
        "module child(input z, input a); endmodule\nmodule top; child u(.z(x), .a(y)); endmodule\n"
    );
}

#[test]
fn sort_named_port_connections_uses_module_order_for_availability() {
    let labels = action_labels_without_diagnostics(
        "module child(input z, input a); endmodule\nmodule top; child u(/*caret*/.z(x), .a(y)); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Sort named port connections"));
}

#[test]
fn sort_named_port_connections_rejects_ordered_connections() {
    let labels = action_labels_without_diagnostics(
        "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.b(y), x); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Sort named port connections"));
}

#[test]
fn add_default_case_item_adds_default_before_endcase() {
    let text = "module top; always_comb case (/*caret*/sel)\n    1'b0: y = 0;\nendcase endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "add_default_case_item").unwrap();
    assert_eq!(
        fixed,
        "module top; always_comb case (sel)\n    1'b0: y = 0;\n    default: ;\nendcase endmodule\n"
    );
}

#[test]
fn add_default_case_item_skips_existing_default() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb case (/*caret*/sel) default: ; endcase endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Add default case item"));
}

#[test]
fn invert_if_else_swaps_branches_and_negates_condition() {
    let text = "module top; always_comb if (/*caret*/a) y = 1; else y = 0; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "invert_if_else").unwrap();
    assert_eq!(fixed, "module top; always_comb if (!(a)) y = 0; else y = 1; endmodule\n");
}

#[test]
fn remove_parentheses_removes_redundant_binary_parens() {
    let text = "module top; assign y = /*caret*/(a + b) + c; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "remove_parentheses").unwrap();
    assert_eq!(fixed, "module top; assign y = a + b + c; endmodule\n");
}

#[test]
fn remove_parentheses_keeps_required_parens() {
    let labels = action_labels_without_diagnostics(
        "module top; assign y = /*caret*/(a + b) * c; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Remove redundant parentheses"));
}

#[test]
fn remove_parentheses_requires_cursor_on_paren() {
    let labels = action_labels_without_diagnostics(
        "module top; assign y = (a /*caret*/+ b) + c; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Remove redundant parentheses"));
}

#[test]
fn merge_nested_if_merges_simple_nested_if() {
    let text = "module top; always_comb if (/*caret*/a) begin if (b) y = 1; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a && b) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_wraps_or_conditions() {
    let text =
        "module top; always_comb if (/*caret*/a || b) begin if (c || d) y = 1; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if ((a || b) && (c || d)) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_merges_multiple_nested_levels() {
    let text = "module top; always_comb if (/*caret*/a) begin if (b) begin if (c) y = 1; end end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a && b && c) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_triggers_from_middle_nested_level() {
    let text = "module top; always_comb if (a) begin if (/*caret*/b) begin if (c) y = 1; end end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a && b && c) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_triggers_from_innermost_nested_level() {
    let text = "module top; always_comb if (a) begin if (b) begin if (/*caret*/c) y = 1; end end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a && b && c) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_merges_mixed_block_and_unbraced_levels() {
    let text = "module top; always_comb if (a) begin if (/*caret*/b) if (c) y = 1; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "merge_nested_if").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a && b && c) y = 1; endmodule\n");
}

#[test]
fn merge_nested_if_requires_no_else_branches() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb if (/*caret*/a) begin if (b) y = 1; else y = 0; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Merge nested if"));
}

#[test]
fn merge_nested_if_rejects_block_with_declarations() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb if (/*caret*/a) begin logic tmp; if (b) y = tmp; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Merge nested if"));
}

#[test]
fn extract_variable_inserts_local_before_statement() {
    let text = "module top; always_comb begin y = /*selection*/a + b/*selection*/; end endmodule\n";
    let fixed = apply_action_without_diagnostics_with_selection(text, "extract_variable").unwrap();
    assert_eq!(
        fixed,
        "module top; always_comb begin logic value = a + b;\ny = value; end endmodule\n"
    );
}

#[test]
fn extract_variable_allows_selection_padding() {
    let text =
        "module top; always_comb begin y =/*selection*/ a + b /*selection*/; end endmodule\n";
    let fixed = apply_action_without_diagnostics_with_selection(text, "extract_variable").unwrap();
    assert_eq!(
        fixed,
        "module top; always_comb begin logic value = a + b;\ny = value ; end endmodule\n"
    );
}

#[test]
fn extract_variable_uses_assignment_lhs_type() {
    let text = "module top; logic [7:0] y, a, b; always_comb begin y = /*selection*/a + b/*selection*/; end endmodule\n";
    let fixed = apply_action_without_diagnostics_with_selection(text, "extract_variable").unwrap();
    assert_eq!(
        fixed,
        "module top; logic [7:0] y, a, b; always_comb begin logic [7:0] value = a + b;\ny = value; end endmodule\n"
    );
}

#[test]
fn extract_variable_from_continuous_assign() {
    let text = "module top; assign y = /*selection*/a + b/*selection*/; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_selection(text, "extract_variable").unwrap();
    assert_eq!(fixed, "module top; wire logic value = a + b;\nassign y = value; endmodule\n");
}

#[test]
fn extract_variable_uses_continuous_assign_lhs_type() {
    let text =
        "module top; logic [7:0] y, a, b; assign y = /*selection*/a + b/*selection*/; endmodule\n";
    let fixed = apply_action_without_diagnostics_with_selection(text, "extract_variable").unwrap();
    assert_eq!(
        fixed,
        "module top; logic [7:0] y, a, b; wire logic [7:0] value = a + b;\nassign y = value; endmodule\n"
    );
}

#[test]
fn extract_variable_requires_selection() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin y = a /*caret*/+ b; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Extract into variable"));
}

#[test]
fn extract_variable_requires_complete_expression_selection() {
    let labels = action_labels_without_diagnostics_with_selection(
        "module top; always_comb begin y = a /*selection*/+/*selection*/ b; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Extract into variable"));
}

#[test]
fn extract_variable_rejects_continuous_assign_lhs() {
    let labels = action_labels_without_diagnostics_with_selection(
        "module top; assign /*selection*/y/*selection*/ = a + b; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Extract into variable"));
}

#[test]
fn extract_variable_requires_block_scope() {
    let labels = action_labels_without_diagnostics_with_selection(
        "module top; always_comb if (a) y = /*selection*/b + c/*selection*/; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Extract into variable"));
}

#[test]
fn pull_assignment_up_converts_if_else_assignment_to_ternary() {
    let text = "module top; always_comb /*caret*/if (a) y = 1; else y = 0; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_up").unwrap();
    assert_eq!(fixed, "module top; always_comb y = a ? 1 : 0; endmodule\n");
}

#[test]
fn pull_assignment_up_converts_else_if_chain_to_nested_ternary() {
    let text =
        "module top; always_comb if (/*caret*/a) y = 1; else if (b) y = 2; else y = 3; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_up").unwrap();
    assert_eq!(fixed, "module top; always_comb y = a ? 1 : b ? 2 : 3; endmodule\n");
}

#[test]
fn pull_assignment_up_triggers_from_else_if_chain_body() {
    let text =
        "module top; always_comb if (a) y = 1; else if (b) /*caret*/y = 2; else y = 3; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_up").unwrap();
    assert_eq!(fixed, "module top; always_comb y = a ? 1 : b ? 2 : 3; endmodule\n");
}

#[test]
fn pull_assignment_up_wraps_conditional_predicate() {
    let text = "module top; always_comb if (a ? b : c) /*caret*/y = 1; else y = 0; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_up").unwrap();
    assert_eq!(fixed, "module top; always_comb y = (a ? b : c) ? 1 : 0; endmodule\n");
}

#[test]
fn pull_assignment_up_requires_single_assignment_branches() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb if (a) begin /*caret*/y = 1; z = 0; end else y = 2; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Pull assignment up"));
}

#[test]
fn pull_assignment_up_rejects_block_with_declarations() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb if (a) begin logic tmp; /*caret*/y = tmp; end else y = 0; endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Pull assignment up"));
}

#[test]
fn pull_assignment_down_converts_ternary_assignment_to_if_else() {
    let text = "module top; always_comb /*caret*/y = a ? 1 : 0; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_down").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a) y = 1; else y = 0; endmodule\n");
}

#[test]
fn pull_assignment_down_converts_nested_ternary_to_else_if_chain() {
    let text = "module top; always_comb /*caret*/y = a ? 1 : b ? 2 : 3; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "pull_assignment_down").unwrap();
    assert_eq!(
        fixed,
        "module top; always_comb if (a) y = 1; else if (b) y = 2; else y = 3; endmodule\n"
    );
}

#[test]
fn unwrap_single_statement_block_unwraps_single_statement() {
    let text = "module top; always_comb if (a) /*caret*/begin y = 1; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "unwrap_single_statement_block").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a) y = 1; endmodule\n");
}

#[test]
fn unwrap_single_statement_block_requires_single_statement() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb /*caret*/begin y = 1; z = 0; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Unwrap single-statement begin/end"));
}

#[test]
fn unwrap_single_statement_block_requires_control_flow_body() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb /*caret*/begin y = 1; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Unwrap single-statement begin/end"));
}

#[test]
fn unwrap_single_statement_block_unwraps_for_body() {
    let text =
        "module top; always_comb for (int i = 0; i < 4; i++) /*caret*/begin y = i; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "unwrap_single_statement_block").unwrap();
    assert_eq!(fixed, "module top; always_comb for (int i = 0; i < 4; i++) y = i; endmodule\n");
}

#[test]
fn wrap_statement_in_begin_end_wraps_statement() {
    let text = "module top; always_comb if (a) /*caret*/y = 1; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "wrap_statement_in_begin_end").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a) begin\n    y = 1;\nend endmodule\n");
}

#[test]
fn wrap_statement_in_begin_end_skips_existing_block() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb if (a) /*caret*/begin y = 1; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Wrap statement in begin/end"));
}

#[test]
fn wrap_statement_in_begin_end_requires_control_flow_body() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/y = 1; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Wrap statement in begin/end"));
}

#[test]
fn wrap_statement_in_begin_end_wraps_for_body() {
    let text = "module top; always_comb for (int i = 0; i < 4; i++) /*caret*/y = i; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "wrap_statement_in_begin_end").unwrap();
    assert_eq!(
        fixed,
        "module top; always_comb for (int i = 0; i < 4; i++) begin\n    y = i;\nend endmodule\n"
    );
}

#[test]
fn expand_postfix_inc_dec_expands_increment() {
    let text = "module top; always_comb begin /*caret*/i++; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "expand_postfix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i = i + 1; end endmodule\n");
}

#[test]
fn expand_prefix_inc_dec_expands_decrement() {
    let text = "module top; always_comb begin /*caret*/--i; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "expand_prefix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i = i - 1; end endmodule\n");
}

#[test]
fn convert_postfix_to_prefix_inc_dec_converts_increment() {
    let text = "module top; always_comb begin /*caret*/i++; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_postfix_to_prefix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin ++i; end endmodule\n");
}

#[test]
fn convert_postfix_to_compound_inc_dec_converts_decrement() {
    let text = "module top; always_comb begin /*caret*/i--; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_postfix_to_compound_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i -= 1; end endmodule\n");
}

#[test]
fn convert_prefix_to_postfix_inc_dec_converts_decrement() {
    let text = "module top; always_comb begin /*caret*/--i; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_prefix_to_postfix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i--; end endmodule\n");
}

#[test]
fn convert_prefix_to_compound_inc_dec_converts_increment() {
    let text = "module top; always_comb begin /*caret*/++i; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_prefix_to_compound_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i += 1; end endmodule\n");
}

#[test]
fn convert_compound_to_postfix_inc_dec_converts_increment() {
    let text = "module top; always_comb begin /*caret*/i += 1; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_compound_to_postfix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i++; end endmodule\n");
}

#[test]
fn convert_compound_to_prefix_inc_dec_converts_decrement() {
    let text = "module top; always_comb begin /*caret*/i -= 1; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_compound_to_prefix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin --i; end endmodule\n");
}

#[test]
fn inc_dec_assists_are_limited_to_other_two_forms() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/i++; end endmodule\n",
    );
    let labels = labels
        .into_iter()
        .filter(|label| label.contains("expression") || label.contains("compound assignment"))
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 3);
    assert!(labels.iter().any(|label| label == "Expand postfix expression"));
    assert!(labels.iter().any(|label| label == "Convert postfix to prefix expression"));
    assert!(labels.iter().any(|label| label == "Convert postfix to compound assignment"));
}

#[test]
fn inc_dec_assists_require_discarded_value_context() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin y = /*caret*/i++; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Expand postfix expression"));
    assert!(!labels.iter().any(|label| label == "Convert postfix to prefix expression"));
    assert!(!labels.iter().any(|label| label == "Convert postfix to compound assignment"));
}

#[test]
fn inc_dec_assists_support_for_loop_steps() {
    let text = "module top; int i; logic y; always_comb for (i = 0; i < 4; /*caret*/i++) y = i; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "expand_postfix_inc_dec").unwrap();
    assert_eq!(
        fixed,
        "module top; int i; logic y; always_comb for (i = 0; i < 4; i = i + 1) y = i; endmodule\n"
    );
}

#[test]
fn compound_inc_dec_can_expand() {
    let text = "module top; always_comb begin /*caret*/i += 1; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "expand_compound_assignment").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i = i + 1; end endmodule\n");
}

#[test]
fn convert_assignment_to_postfix_inc_dec_converts_increment() {
    let text = "module top; always_comb begin /*caret*/i = i + 1; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_assignment_to_postfix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin i++; end endmodule\n");
}

#[test]
fn convert_assignment_to_prefix_inc_dec_converts_decrement() {
    let text = "module top; always_comb begin /*caret*/i = i - 1; end endmodule\n";
    let fixed =
        apply_action_without_diagnostics(text, "convert_assignment_to_prefix_inc_dec").unwrap();
    assert_eq!(fixed, "module top; always_comb begin --i; end endmodule\n");
}

#[test]
fn convert_assignment_inc_dec_requires_same_lhs_and_one() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/i = j + 1; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert assignment to postfix expression"));
    assert!(!labels.iter().any(|label| label == "Convert assignment to prefix expression"));
}

#[test]
fn convert_assignment_inc_dec_requires_discarded_value_context() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin y = (/*caret*/i = i + 1); end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Convert assignment to postfix expression"));
    assert!(!labels.iter().any(|label| label == "Convert assignment to prefix expression"));
}

#[test]
fn convert_compound_inc_dec_requires_one() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/i += 2; end endmodule\n",
    );
    assert!(
        !labels.iter().any(|label| label == "Convert compound assignment to postfix expression")
    );
    assert!(
        !labels.iter().any(|label| label == "Convert compound assignment to prefix expression")
    );
}

#[test]
fn expand_compound_assignment_expands_assignment() {
    let text = "module top; always_comb begin /*caret*/a += b; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "expand_compound_assignment").unwrap();
    assert_eq!(fixed, "module top; always_comb begin a = a + b; end endmodule\n");
}

#[test]
fn collapse_compound_assignment_collapses_assignment() {
    let text = "module top; always_comb begin /*caret*/a = a + b; end endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "collapse_compound_assignment").unwrap();
    assert_eq!(fixed, "module top; always_comb begin a += b; end endmodule\n");
}

#[test]
fn collapse_compound_assignment_requires_same_lhs() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/a = c + b; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Collapse compound assignment"));
}

#[test]
fn expand_compound_assignment_skips_plain_assignment() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb begin /*caret*/a = b; end endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Expand compound assignment"));
}

#[test]
fn apply_de_morgan_rewrites_parenthesized_logical_expression() {
    let text = "module top; assign y = /*caret*/!(a && b); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "apply_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = !a || !b; endmodule\n");
}

#[test]
fn apply_de_morgan_rewrites_logical_chain() {
    let text = "module top; assign y = /*caret*/!(a && b && c); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "apply_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = !a || !b || !c; endmodule\n");
}

#[test]
fn apply_de_morgan_inverts_comparison_operators() {
    let text = "module top; assign y = /*caret*/!(a == b || c != d || e <= f); endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "apply_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = a != b && c == d && e > f; endmodule\n");
}

#[test]
fn apply_de_morgan_triggers_across_if_condition() {
    let text = "module top; always_comb if (!(a == b /*caret*/|| c != d)) y = 1; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "apply_de_morgan").unwrap();
    assert_eq!(fixed, "module top; always_comb if (a != b && c == d) y = 1; endmodule\n");
}

#[test]
fn factor_de_morgan_rewrites_negated_operands() {
    let text = "module top; assign y = !a /*caret*/|| !b; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "factor_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = !(a && b); endmodule\n");
}

#[test]
fn factor_de_morgan_inverts_comparison_operators() {
    let text = "module top; assign y = a == b /*caret*/&& c < d; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "factor_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = !(a != b || c >= d); endmodule\n");
}

#[test]
fn factor_de_morgan_rewrites_logical_chain() {
    let text = "module top; assign y = a /*caret*/|| b || c; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "factor_de_morgan").unwrap();
    assert_eq!(fixed, "module top; assign y = !(!a && !b && !c); endmodule\n");
}

#[test]
fn factor_de_morgan_triggers_across_if_condition() {
    let text = "module top; always_comb if (a == b /*caret*/&& c < d) y = 1; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "factor_de_morgan").unwrap();
    assert_eq!(fixed, "module top; always_comb if (!(a != b || c >= d)) y = 1; endmodule\n");
}

#[test]
fn factor_de_morgan_triggers_on_if_condition_operand() {
    let text = "module top; always_comb if (a == /*caret*/b && c < d) y = 1; endmodule\n";
    let fixed = apply_action_without_diagnostics(text, "factor_de_morgan").unwrap();
    assert_eq!(fixed, "module top; always_comb if (!(a != b || c >= d)) y = 1; endmodule\n");
}

#[test]
fn factor_de_morgan_requires_cursor_on_logical_operator() {
    let labels =
        action_labels_without_diagnostics("module top; assign y = /*caret*/!a || b; endmodule\n");
    assert!(!labels.iter().any(|label| label == "Factor De Morgan's law"));
}

#[test]
fn apply_de_morgan_requires_parenthesized_logical_expression() {
    let labels =
        action_labels_without_diagnostics("module top; assign y = /*caret*/!a; endmodule\n");
    assert!(!labels.iter().any(|label| label == "Apply De Morgan's law"));
}

#[test]
fn expected_token_repair_inserts_missing_semicolon() {
    let text = "module top;\nlogic a/*caret*/\nendmodule\n";
    let fixed = apply_action(text, RepairKind::InsertExpectedToken).unwrap();
    assert_eq!(fixed, "module top;\nlogic a;\nendmodule\n");
}

#[test]
fn expected_token_repair_uses_diagnostic_range() {
    let text = "/*caret*/module top;\nlogic a\nendmodule\n";
    let clean_text = text.replace("/*caret*/", "");
    let diagnostic_offset = TextSize::from(clean_text.find("\nendmodule").unwrap() as u32);
    let (db, file_id, offset) = db_with_file(text);
    let mut diagnostic = diagnostic_for_repair(RepairKind::InsertExpectedToken);
    diagnostic.range = Some(TextRange::empty(diagnostic_offset));
    let actions = code_action(
        &db,
        file_id,
        TextRange::empty(offset),
        CodeActionDiagnostics { items: vec![diagnostic] },
        CodeActionResolveStrategy::All,
    );
    let action =
        actions.into_iter().find(|action| action.id.name == "insert_expected_token").unwrap();
    let mut fixed = clean_text;
    let edit = action.source_change.unwrap().text_edits.remove(&file_id).unwrap();
    edit.apply(&mut fixed);

    assert_eq!(fixed, "module top;\nlogic a;\nendmodule\n");
}

#[test]
fn expected_token_repair_requires_diagnostic() {
    let text = "module top;\nlogic a/*caret*/\nendmodule\n";
    let labels = action_labels_without_diagnostics(text);
    assert!(!labels.iter().any(|label| label.starts_with("Insert missing ")));
}
