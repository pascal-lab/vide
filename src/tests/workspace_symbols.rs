use super::*;

#[test]
fn workspace_symbol_finds_symbols_across_files() {
    let files = [
        (
            "top.sv",
            "\
module top;
  logic shared_signal;
  child u_child();
endmodule
",
        ),
        (
            "child.sv",
            "\
module child;
endmodule
",
        ),
    ];
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &files,
    );

    // Workspace symbols use the same workspace/VFS readiness boundary.  A
    // document diagnostic result id gives the test an explicit synchronization
    // point before asserting on symbol contents.
    let _ = request_document_diagnostics(&client, uris[0].clone(), 180);

    let symbols_id = lsp_server::RequestId::from(181);
    client
        .sender
        .send(Message::Request(Request::new(
            symbols_id.clone(),
            WorkspaceSymbolRequest::METHOD.to_string(),
            WorkspaceSymbolParams {
                query: "top shared".to_string(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let symbols: Option<WorkspaceSymbolResponse> =
        recv_response(&client, symbols_id, "workspaceSymbol");
    let WorkspaceSymbolResponse::Flat(symbols) =
        symbols.expect("workspaceSymbol should return a result")
    else {
        panic!("workspaceSymbol should return flat SymbolInformation results");
    };

    assert!(
        symbols
            .iter()
            .any(|symbol| symbol.name == "shared_signal" && symbol.location.uri == uris[0]),
        "qualified query should match symbols by container and name: {symbols:?}"
    );

    let symbols_id = lsp_server::RequestId::from(182);
    client
        .sender
        .send(Message::Request(Request::new(
            symbols_id.clone(),
            WorkspaceSymbolRequest::METHOD.to_string(),
            WorkspaceSymbolParams {
                query: "child".to_string(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let symbols: Option<WorkspaceSymbolResponse> =
        recv_response(&client, symbols_id, "workspaceSymbol");
    let WorkspaceSymbolResponse::Flat(symbols) =
        symbols.expect("workspaceSymbol should return a result")
    else {
        panic!("workspaceSymbol should return flat SymbolInformation results");
    };

    assert!(
        symbols.iter().any(|symbol| symbol.name == "child" && symbol.location.uri == uris[1]),
        "workspaceSymbol should find module symbols across files: {symbols:?}"
    );
    assert!(
        symbols.iter().any(|symbol| symbol.name == "u_child" && symbol.location.uri == uris[0]),
        "workspaceSymbol should include matching nested symbols: {symbols:?}"
    );

    shutdown_test_server(&client, server_thread);
}
