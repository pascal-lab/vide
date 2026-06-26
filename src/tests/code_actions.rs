use super::*;

#[test]
fn code_action_request_returns_ordered_connection_refactor_without_diagnostics() {
    let text = "\
module ca_leaf(input clk, input rst_n, output done);
endmodule

module top;
  logic clk, rst_n, done;
  ca_leaf convert_ports_only (clk, rst_n, done);
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);
    let diagnostics_id = lsp_server::RequestId::from(199);
    client
        .sender
        .send(Message::Request(Request::new(
            diagnostics_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let _ = recv_document_diagnostics(&client, diagnostics_id);

    let actions = request_code_actions(
        &client,
        uri,
        text,
        "convert_ports_only (clk",
        CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![CodeActionKind::REFACTOR_REWRITE]),
            trigger_kind: None,
        },
        200,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "Convert ordered port connections to named connections"),
        "expected ordered port conversion refactor, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn code_action_request_returns_extract_variable_for_selected_expression() {
    let text = "\
module top;
  always_comb begin
    y = a + b;
  end
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);

    let actions = request_code_actions_with_range(
        &client,
        uri,
        range_of(text, "a + b"),
        CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![CodeActionKind::REFACTOR_EXTRACT]),
            trigger_kind: None,
        },
        201,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "Extract into variable"),
        "expected extract variable refactor, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn code_action_request_returns_extract_variable_for_selected_continuous_assign_rhs() {
    let text = "\
module top (
    c,
    led0
);
    input wire c;
    output led0;
    reg led0;

    assign led0 = c * 2 + c;
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);

    let actions = request_code_actions_with_range(
        &client,
        uri,
        range_of(text, "c * 2 + c"),
        CodeActionContext {
            diagnostics: Vec::new(),
            only: Some(vec![CodeActionKind::REFACTOR_EXTRACT]),
            trigger_kind: None,
        },
        202,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "Extract into variable"),
        "expected extract variable refactor, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn code_action_request_uses_server_diagnostics_when_client_diagnostic_has_no_data() {
    let text = "\
module ca_leaf(input clk, input rst_n, output done);
endmodule

module top;
  logic clk, rst_n, done;
  ca_leaf mixed_ports (clk, .rst_n(rst_n), .done(done));
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_configured_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);

    let (_result_id, mut diagnostics) = request_document_diagnostics_until(
        &client,
        uri.clone(),
        210,
        |_result_id, diagnostics| !diagnostics.is_empty(),
        "semantic diagnostics for code action",
    );
    for diagnostic in &mut diagnostics {
        diagnostic.data = None;
    }

    let actions = request_code_actions(
        &client,
        uri,
        text,
        "clk, .rst_n",
        CodeActionContext {
            diagnostics,
            only: Some(vec![CodeActionKind::QUICKFIX]),
            trigger_kind: None,
        },
        211,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "Convert ordered port connections to named connections"),
        "expected mixed connection quickfix without client diagnostic data, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn code_action_request_returns_expected_token_quickfix_for_parse_diagnostic() {
    let text = "\
module top;
  logic a
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);

    let diagnostics_id = lsp_server::RequestId::from(212);
    client
        .sender
        .send(Message::Request(Request::new(
            diagnostics_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let (_result_id, diagnostics) = recv_document_diagnostics(&client, diagnostics_id);
    assert!(
        diagnostics.iter().any(|diag| diag.message == "expected ';'"),
        "expected parse diagnostic for missing semicolon, got {diagnostics:?}"
    );

    let actions = request_code_actions(
        &client,
        uri,
        text,
        "\nendmodule",
        CodeActionContext {
            diagnostics,
            only: Some(vec![CodeActionKind::QUICKFIX]),
            trigger_kind: None,
        },
        213,
    );
    let titles = code_action_titles(&actions);

    assert!(
        titles.iter().any(|title| title == "Insert missing ';'"),
        "expected expected-token quickfix, got {titles:?}"
    );

    shutdown_test_server(&client, server_thread);
}
