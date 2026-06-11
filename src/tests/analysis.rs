use super::*;

#[test]
fn clearing_open_document_updates_analysis_state() {
    let text = "module stale_after_clear;\nendmodule\n";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(ClientCapabilities::default(), UserConfig::default(), text);

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: uri.clone(), version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: String::new(),
                }],
            },
        )))
        .unwrap();

    let symbols_id = lsp_server::RequestId::from(180);
    client
        .sender
        .send(Message::Request(Request::new(
            symbols_id.clone(),
            DocumentSymbolRequest::METHOD.to_string(),
            DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let symbols: Option<DocumentSymbolResponse> =
        recv_response(&client, symbols_id, "documentSymbol");
    let symbol_count = match symbols {
        Some(DocumentSymbolResponse::Nested(symbols)) => symbols.len(),
        Some(DocumentSymbolResponse::Flat(symbols)) => symbols.len(),
        None => 0,
    };

    assert_eq!(symbol_count, 0, "cleared documents must not expose stale symbols");

    shutdown_test_server(&client, server_thread);
}
