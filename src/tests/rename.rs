use super::*;

#[test]
fn workspace_file_rename_updates_symbol_and_definition_locations() {
    let temp_dir = TempDir::new("workspace-file-rename");
    fs::write(temp_dir.path().join("vide.toml"), DEFAULT_TEST_CONFIG).unwrap();
    let top_text = "\
module top;
  child u_child();
endmodule
";
    let child_text = "\
module child;
endmodule
";
    let top_path = temp_dir.path().join("top.sv");
    let old_child_path = temp_dir.path().join("old_child.sv");
    fs::write(&top_path, top_text).unwrap();
    fs::write(&old_child_path, child_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let old_child_uri = to_proto::url_from_abs_path(old_child_path.as_path()).unwrap();
    let new_child_path = temp_dir.path().join("child.sv");
    let new_child_uri = to_proto::url_from_abs_path(new_child_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);

    fs::rename(&old_child_path, &new_child_path).unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeWatchedFiles::METHOD.to_string(),
            lsp_types::DidChangeWatchedFilesParams {
                changes: vec![
                    FileEvent::new(old_child_uri.clone(), FileChangeType::DELETED),
                    FileEvent::new(new_child_uri.clone(), FileChangeType::CREATED),
                ],
            },
        )))
        .unwrap();

    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    let mut saw_new_symbol_location = false;
    let mut last_symbol_locations = Vec::new();
    let mut attempt = 0;
    while Instant::now() < deadline {
        attempt += 1;
        let symbols_id = lsp_server::RequestId::from(900 + attempt);
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
            recv_response(&client, symbols_id, "workspaceSymbol after file rename");
        let WorkspaceSymbolResponse::Flat(symbols) =
            symbols.expect("workspaceSymbol should return a result")
        else {
            panic!("workspaceSymbol should return flat SymbolInformation results");
        };
        last_symbol_locations =
            symbols.iter().map(|symbol| symbol.location.uri.clone()).collect::<Vec<_>>();

        if symbols
            .iter()
            .any(|symbol| symbol.name == "child" && symbol.location.uri == new_child_uri)
            && !symbols
                .iter()
                .any(|symbol| symbol.name == "child" && symbol.location.uri == old_child_uri)
        {
            saw_new_symbol_location = true;
            break;
        }

        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        saw_new_symbol_location,
        "workspace symbol should move from old_child.sv to child.sv after file rename: {last_symbol_locations:?}"
    );

    let definition_uris = request_goto_definition_uris(&client, top_uri, top_text, "child u", 1901);
    assert!(
        definition_uris.contains(&new_child_uri),
        "definition should target renamed child.sv: {definition_uris:?}"
    );
    assert!(
        !definition_uris.contains(&old_child_uri),
        "definition should not target stale old_child.sv after rename: {definition_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unconfigured_workspace_rename_updates_file_local_symbol() {
    let temp_dir = TempDir::new("unconfigured-index-rename-local");
    let top_path = temp_dir.path().join("top.sv");
    let top_text = "module top;\n  logic sig;\n  always_comb sig = sig;\nendmodule\n";
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let edit = request_rename(&client, top_uri.clone(), top_text, "sig = sig", "renamed_sig", 2)
        .expect("best-effort local rename should return an edit");

    let Some(lsp_types::DocumentChanges::Edits(document_edits)) = edit.document_changes else {
        panic!("rename should use document edits: {edit:?}");
    };
    assert_eq!(document_edits.len(), 1, "local rename should stay in one file: {document_edits:?}");
    let document_edit = &document_edits[0];
    assert_eq!(document_edit.text_document.uri, top_uri);
    let text_edits = document_edit
        .edits
        .iter()
        .map(|edit| match edit {
            lsp_types::OneOf::Left(edit) => edit,
            lsp_types::OneOf::Right(_) => panic!("rename should not emit annotated edits"),
        })
        .collect::<Vec<_>>();
    assert_eq!(text_edits.len(), 3, "rename should update declaration and both uses");
    assert!(text_edits.iter().all(|edit| edit.new_text == "renamed_sig"));

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unconfigured_workspace_rename_rejects_cross_file_symbol() {
    let temp_dir = TempDir::new("unconfigured-index-rename-cross-file");
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.sv");
    let top_text = "module top;\n  child u();\nendmodule\n";
    fs::write(&child_path, "module child;\nendmodule\n").unwrap();
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        UserConfig::default(),
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let response =
        request_rename_response(&client, top_uri, top_text, "child u", "renamed_child", 2);
    let error = response.error.expect("cross-file best-effort rename should be rejected");
    assert!(
        error
            .message
            .contains("This rename can affect other files. Add vide.toml to make the editable project scope explicit."),
        "unexpected rename error: {error:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn configured_workspace_rename_updates_cross_file_symbol() {
    let child_text = "module child;\nendmodule\n";
    let top_text = "module top;\n  child u();\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("child.sv", child_text), ("top.sv", top_text)],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let edit = request_rename(&client, top_uri, top_text, "child u", "renamed_child", 2)
        .expect("configured cross-file rename should return an edit");

    let Some(lsp_types::DocumentChanges::Edits(document_edits)) = edit.document_changes else {
        panic!("rename should use document edits: {edit:?}");
    };
    assert_eq!(
        document_edits.len(),
        2,
        "cross-file rename should edit both declaration and use sites: {document_edits:?}"
    );
    assert!(
        document_edits.iter().any(|edit| edit.text_document.uri == child_uri),
        "configured cross-file rename should edit child declaration: {document_edits:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn configured_workspace_expanded_rename_command_updates_chain() {
    let child_text = "module child(input a);\nendmodule\n";
    let top_text = "module top(input a);\n  child u(.a(a));\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("child.sv", child_text), ("top.sv", top_text)],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let text_document_position = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: top_uri.clone() },
        position: position_of(top_text, "a);\n  child"),
    };
    let info_response = request_execute_command_response(
        &client,
        RENAME_EXPANSION_INFO_COMMAND,
        vec![
            serde_json::to_value(RenameExpansionInfoParams {
                text_document_position: text_document_position.clone(),
            })
            .unwrap(),
        ],
        2,
    );
    assert!(info_response.error.is_none(), "rename info returned error: {:?}", info_response.error);
    let info: RenameExpansionInfoResult =
        serde_json::from_value(info_response.result.unwrap()).unwrap();
    assert_eq!(info.additional_symbols, 1);

    let rename_response = request_execute_command_response(
        &client,
        EXPANDED_RENAME_COMMAND,
        vec![
            serde_json::to_value(ExpandedRenameParams {
                text_document_position,
                new_name: "renamed".to_owned(),
            })
            .unwrap(),
        ],
        3,
    );
    assert!(
        rename_response.error.is_none(),
        "recursive rename returned error: {:?}",
        rename_response.error
    );
    let edit: lsp_types::WorkspaceEdit =
        serde_json::from_value(rename_response.result.unwrap()).unwrap();
    let Some(lsp_types::DocumentChanges::Edits(document_edits)) = edit.document_changes else {
        panic!("recursive rename should use document edits: {edit:?}");
    };
    assert!(
        document_edits.iter().any(|edit| edit.text_document.uri == top_uri),
        "recursive rename should edit top file: {document_edits:?}"
    );
    assert!(
        document_edits.iter().any(|edit| edit.text_document.uri == child_uri),
        "recursive rename should edit child file: {document_edits:?}"
    );
    assert!(
        document_edits.iter().flat_map(|edit| edit.edits.iter()).any(|edit| {
            matches!(edit, lsp_types::OneOf::Left(edit) if edit.new_text == "renamed")
        }),
        "recursive rename should contain rename edits: {document_edits:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn configured_workspace_rename_conflict_info_command_reports_conflicts() {
    let text = "module top;\n  logic a;\n  logic b;\n  assign a = b;\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("top.sv", text)],
    );
    let uri = uris[0].clone();
    let _ = request_document_diagnostics(&client, uri.clone(), 1);

    let response = request_execute_command_response(
        &client,
        RENAME_CONFLICT_INFO_COMMAND,
        vec![
            serde_json::to_value(RenameConflictInfoParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, "b;"),
                },
                new_name: "a".to_owned(),
                recursive: false,
            })
            .unwrap(),
        ],
        2,
    );
    assert!(response.error.is_none(), "rename collision info returned error: {:?}", response.error);
    let info: RenameConflictInfoResult = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(info.conflicts, 1);

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unconfigured_workspace_expanded_rename_command_rejects_cross_file_chain() {
    let child_text = "module child(input a);\nendmodule\n";
    let top_text = "module top(input a);\n  child u(.a(a));\nendmodule\n";
    let (_temp_dir, client, server_thread, uris) = setup_multi_file_diagnostics_test_inner(
        ClientCapabilities::default(),
        UserConfig::default(),
        &[("child.sv", child_text), ("top.sv", top_text)],
        false,
    );
    let top_uri = uris[1].clone();
    let _ = request_document_diagnostics(&client, top_uri.clone(), 1);

    let response = request_execute_command_response(
        &client,
        EXPANDED_RENAME_COMMAND,
        vec![
            serde_json::to_value(ExpandedRenameParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: top_uri },
                    position: position_of(top_text, "a);\n  child"),
                },
                new_name: "renamed".to_owned(),
            })
            .unwrap(),
        ],
        2,
    );
    let error = response.error.expect("recursive cross-file rename should be rejected");
    assert!(
        error
            .message
            .contains("This rename can affect other files. Add vide.toml to make the editable project scope explicit."),
        "unexpected recursive rename error: {error:?}"
    );

    shutdown_test_server(&client, server_thread);
}
