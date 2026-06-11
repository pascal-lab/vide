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
            FixtureAction::Action { name, label } => {
                if self.source.contains("/*selection*/") {
                    if label.is_some() {
                        panic!("selection fixture {} cannot specify label", path.display());
                    }
                    apply_action_without_diagnostics_with_selection(&self.source, name)
                } else {
                    match label {
                        Some(label) => {
                            apply_action_without_diagnostics_with_label(&self.source, name, label)
                        }
                        None => apply_action_without_diagnostics(&self.source, name),
                    }
                }
            }
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
fn split_declaration_declarators_requires_multiple_declarators() {
    let labels = action_labels_without_diagnostics("module top; /*caret*/logic a; endmodule\n");
    assert!(!labels.iter().any(|label| label == "Split declaration"));
}

#[test]
fn sort_named_parameter_assignments_rejects_mixed_assignments() {
    let labels = action_labels_without_diagnostics(
        "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/.B(2), 1) u(); endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Sort named parameter assignments"));
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
fn add_default_case_item_skips_existing_default() {
    let labels = action_labels_without_diagnostics(
        "module top; always_comb case (/*caret*/sel) default: ; endcase endmodule\n",
    );
    assert!(!labels.iter().any(|label| label == "Add default case item"));
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
