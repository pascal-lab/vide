use super::*;

#[test]
fn verilog_2005_memory_lsp_requests_handle_supported_constructs() {
    let file_text = "\
module child(input wire a, output wire y);
endmodule

primitive udp_and(out, in);
  output out;
  input in;
  table
    1 : 1;
  endtable
endprimitive

module top(input wire clk);
  wire sig;
  child u_child(.a(sig), .y());

  task automatic do_task;
    input reg t_in;
    begin
      sig = t_in;
    end
  endtask

  generate
    genvar i;
    for (i = 0; i < 1; i = i + 1) begin : g_loop
      wire lane;
    end
  endgenerate

  specify
    specparam T_SETUP = 1;
  endspecify

  initial begin : blk
    do_task(sig);
    $display(\"%0d\", sig);
  end
endmodule

config cfg_top;
  design work.top;
endconfig
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(ClientCapabilities::default(), UserConfig::default(), file_text);
    let text_document = TextDocumentIdentifier { uri: uri.clone() };

    let diagnostics_id = lsp_server::RequestId::from(100);
    client
        .sender
        .send(Message::Request(Request::new(
            diagnostics_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: text_document.clone(),
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let diagnostics: DocumentDiagnosticReportResult =
        recv_response(&client, diagnostics_id, "documentDiagnostic");
    if let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) =
        diagnostics
    {
        assert!(
            report
                .full_document_diagnostic_report
                .items
                .iter()
                .all(|diag| diag.source.as_deref() != Some("vide")),
            "document diagnostics should not include removed Vide model diagnostics"
        );
    }

    let symbols_id = lsp_server::RequestId::from(101);
    client
        .sender
        .send(Message::Request(Request::new(
            symbols_id.clone(),
            DocumentSymbolRequest::METHOD.to_string(),
            DocumentSymbolParams {
                text_document: text_document.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let symbols: Option<DocumentSymbolResponse> =
        recv_response(&client, symbols_id, "documentSymbol");
    assert!(symbols.is_some(), "documentSymbol should return a result");

    let tokens_id = lsp_server::RequestId::from(102);
    client
        .sender
        .send(Message::Request(Request::new(
            tokens_id.clone(),
            SemanticTokensFullRequest::METHOD.to_string(),
            SemanticTokensParams {
                text_document: text_document.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let tokens: Option<SemanticTokensResult> =
        recv_response(&client, tokens_id, "semanticTokens/full");
    assert!(tokens.is_some(), "semanticTokens/full should return a result");

    let folding_id = lsp_server::RequestId::from(103);
    client
        .sender
        .send(Message::Request(Request::new(
            folding_id.clone(),
            FoldingRangeRequest::METHOD.to_string(),
            FoldingRangeParams {
                text_document: text_document.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let folds: Option<Vec<FoldingRange>> = recv_response(&client, folding_id, "foldingRange");
    assert!(folds.is_some_and(|folds| !folds.is_empty()), "folding ranges expected");

    let hover_id = lsp_server::RequestId::from(104);
    client
        .sender
        .send(Message::Request(Request::new(
            hover_id.clone(),
            HoverRequest::METHOD.to_string(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: text_document.clone(),
                    position: position_of(file_text, "g_loop"),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();
    let hover: Option<Hover> = recv_response(&client, hover_id, "hover");
    assert!(hover.is_some(), "hover should return a result for generate label");

    let definition_id = lsp_server::RequestId::from(105);
    client
        .sender
        .send(Message::Request(Request::new(
            definition_id.clone(),
            GotoDefinition::METHOD.to_string(),
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document,
                    position: position_of(file_text, "sig), .y"),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let definition: Option<GotoDefinitionResponse> =
        recv_response(&client, definition_id, "definition");
    assert!(definition.is_some(), "definition should return a result for sig reference");

    shutdown_test_server(&client, server_thread);
}
