use super::*;

#[test]
fn completion_returns_top_level_module_for_prefix() {
    let text = "mo\n";
    let (_temp_dir, client, server_thread, uri) = setup_configured_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        text,
    );
    let _ = request_document_diagnostics(&client, uri.clone(), 1);

    let request_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            CompletionRequest::METHOD.to_string(),
            CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, "\n"),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        )))
        .unwrap();
    let response = recv_raw_response(&client, request_id, "completion");
    assert!(response.error.is_none(), "completion returned error: {:?}", response.error);
    let result = response.result.expect("completion response missing result");
    let completion: CompletionResponse =
        serde_json::from_value(result).expect("invalid completion response");
    let labels = match completion {
        CompletionResponse::Array(items) => {
            items.into_iter().map(|item| item.label).collect::<Vec<_>>()
        }
        CompletionResponse::List(list) => {
            list.items.into_iter().map(|item| item.label).collect::<Vec<_>>()
        }
    };
    assert!(labels.iter().any(|label| label == "module"), "module completion missing: {labels:?}");

    shutdown_test_server(&client, server_thread);
}

#[test]
fn completion_returns_module_member_keyword() {
    let text = "module m;\n  al\nendmodule\n";
    let (_temp_dir, client, server_thread, uri) = setup_configured_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        text,
    );
    let _ = request_document_diagnostics(&client, uri.clone(), 3);

    let labels = request_completion_labels(&client, uri, text, "al", 4);
    assert!(
        labels.iter().any(|label| label == "always"),
        "module member completion missing: {labels:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn manifest_completion_offers_schema_keys() {
    let text = "sou\n";
    let temp_dir = TempDir::new("manifest-completion");
    let manifest_path = temp_dir.path().join("vide.toml");
    fs::write(&manifest_path, text).unwrap();
    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();
    open_test_document(&client, uri.clone(), text);

    let labels = request_completion_labels(&client, uri, text, "sou", 2);
    assert!(
        labels.iter().any(|label| label == "sources"),
        "manifest key completion missing: {labels:?}"
    );

    shutdown_test_server(&client, server_thread);
}
