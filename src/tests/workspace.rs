use super::*;

#[test]
fn project_manifest_is_not_diagnosed_as_systemverilog() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("manifest-diagnostics");
    let manifest_text = "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\n";
    let manifest_path = temp_dir.path().join("vide.toml");
    fs::write(&manifest_path, manifest_text).unwrap();
    fs::create_dir_all(temp_dir.path().join("rtl")).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let opt = Opt {
        process_name: "vide-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
        profile_trace: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: manifest_uri.clone(),
                    language_id: "toml".to_string(),
                    version: 1,
                    text: manifest_text.to_string(),
                },
            },
        )))
        .unwrap();

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: manifest_uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: String::new(),
                }],
            },
        )))
        .unwrap();

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: manifest_uri.clone(),
                    version: 3,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: manifest_text.to_string(),
                }],
            },
        )))
        .unwrap();

    let request_id = lsp_server::RequestId::from(1);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: manifest_uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let (_, diagnostics) = recv_document_diagnostics(&client, request_id);
    assert!(diagnostics.is_empty(), "manifest must not receive slang diagnostics: {diagnostics:?}");

    shutdown_test_server(&client, server_thread);
}
