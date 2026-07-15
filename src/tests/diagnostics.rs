use super::*;

#[test]
fn default_diagnostics_warn_on_port_width_mismatch() {
    let text = "\
module width_child(input logic [3:0] a);
endmodule

module top;
  logic [7:0] wide;
  width_child u(.a(wide));
endmodule
";
    let (_temp_dir, client, server_thread, uri) = setup_configured_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        text,
    );

    let (_result_id, diagnostics) = request_document_diagnostics_until(
        &client,
        uri,
        190,
        |_result_id, diagnostics| {
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic_option(diagnostic) == Some("port-width-trunc"))
        },
        "default semantic diagnostics",
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic_option(diagnostic) == Some("port-width-trunc")),
        "expected default port width warning, got {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn explicit_empty_slang_warnings_suppress_port_width_mismatch_warning() {
    let text = "\
module width_child(input logic [3:0] a);
endmodule

module top;
  logic [7:0] wide;
  width_child u(.a(wide));
endmodule
";
    let mut user_config = UserConfig::default();
    user_config.diagnostics.slang.warnings = Vec::new();
    let (_temp_dir, client, server_thread, uri) =
        setup_configured_diagnostics_test(ClientCapabilities::default(), user_config, text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 191);

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic_option(diagnostic) != Some("port-width-trunc")),
        "explicit empty warnings should suppress port width warning, got {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn pull_capable_client_does_not_receive_duplicate_publish_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let file_text = "module broken(;\nendmodule\n";
    let temp_dir = TempDir::new("pull-diagnostics-no-publish");
    let file_path = temp_dir.path().join("broken.sv");
    let readiness_path = temp_dir.path().join("readiness.sv");
    fs::write(&readiness_path, "module readiness;\nendmodule\n").unwrap();
    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, UserConfig::default());
    let uri = to_proto::url_from_abs_path(file_path.as_path()).unwrap();
    let readiness_uri = to_proto::url_from_abs_path(readiness_path.as_path()).unwrap();

    // Establish workspace readiness before opening the document.  This keeps
    // the publishDiagnostics observation below intact: no notification for the
    // target document can be consumed as part of readiness synchronization.
    let _ = request_document_diagnostics(&client, readiness_uri, 100);
    open_test_document(&client, uri.clone(), file_text);

    let request_id = lsp_server::RequestId::from(1);
    let request = Request::new(
        request_id.clone(),
        DocumentDiagnosticRequest::METHOD.to_string(),
        DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        },
    );
    client.sender.send(Message::Request(request)).unwrap();

    let mut pull_diagnostics = None;
    let mut saw_publish_diagnostics = false;
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;

    while Instant::now() < deadline && pull_diagnostics.is_none() {
        let timeout = deadline.saturating_duration_since(Instant::now());
        let message = client.receiver.recv_timeout(timeout).unwrap();

        match message {
            Message::Response(response) if response.id == request_id => {
                assert!(response.error.is_none(), "{:?}", response.error);
                let result = serde_json::from_value::<DocumentDiagnosticReportResult>(
                    response.result.unwrap(),
                )
                .unwrap();
                let items = match result {
                    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
                        report,
                    )) => report.full_document_diagnostic_report.items,
                    other => panic!("unexpected diagnostic response: {other:?}"),
                };
                pull_diagnostics = Some(items);
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD =>
            {
                let params =
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap();
                if params.uri == uri {
                    saw_publish_diagnostics = true;
                }
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD =>
            {
                let _ = serde_json::from_value::<ProgressParams>(notification.params).unwrap();
            }
            Message::Request(request) => {
                panic!("unexpected server request during diagnostics test: {request:?}");
            }
            _ => {}
        }
    }

    let pull_diagnostics = pull_diagnostics.expect("documentDiagnostic response not received");
    assert!(!pull_diagnostics.is_empty(), "expected pulled diagnostics");
    assert!(
        pull_diagnostics.iter().any(|diag| !diag.message.is_empty()),
        "expected pulled diagnostic message"
    );
    assert!(!saw_publish_diagnostics, "pull-capable client should not receive publishDiagnostics");

    let quiet_until = Instant::now() + Duration::from_millis(500);
    while Instant::now() < quiet_until {
        let timeout = quiet_until.saturating_duration_since(Instant::now());
        match client.receiver.recv_timeout(timeout) {
            Ok(Message::Notification(notification))
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD =>
            {
                let params =
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap();
                assert_ne!(
                    params.uri, uri,
                    "pull-capable client should not receive publishDiagnostics"
                );
            }
            Ok(Message::Notification(notification))
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Ok(other) => {
                panic!("unexpected message after pull diagnostics response: {other:?}");
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => break,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                panic!("test client disconnected unexpectedly");
            }
        }
    }

    shutdown_test_server(&client, server_thread);
}

#[test]
fn legacy_client_receives_publish_diagnostics() {
    let (_temp_dir, client, server_thread, uri) = setup_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
        "module broken(;\nendmodule\n",
    );
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;

    while Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD =>
            {
                let params =
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap();
                if params.uri == uri {
                    assert!(!params.diagnostics.is_empty(), "expected published diagnostics");
                    shutdown_test_server(&client, server_thread);
                    return;
                }
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Request(request) => {
                panic!("unexpected server request during diagnostics test: {request:?}");
            }
            _ => {}
        }
    }

    panic!("publishDiagnostics notification not received");
}

#[test]
fn semantic_diagnostics_can_be_disabled() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let user_config = UserConfig {
        diagnostics: crate::config::user_config::DiagnosticsUserConfig {
            semantic: crate::config::user_config::DiagnosticsPhaseUserConfig { enable: false },
            ..Default::default()
        },
        ..UserConfig::default()
    };
    let file_text = "\
module child(input logic a, input logic b);
endmodule

module top;
  logic sig;
  child u(.a(sig));
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_configured_diagnostics_test(pull_caps, user_config, file_text);

    let request_id = lsp_server::RequestId::from(1);
    let request = Request::new(
        request_id.clone(),
        DocumentDiagnosticRequest::METHOD.to_string(),
        DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        },
    );
    client.sender.send(Message::Request(request)).unwrap();

    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    while Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Response(response) if response.id == request_id => {
                assert!(response.error.is_none(), "{:?}", response.error);
                let result = serde_json::from_value::<DocumentDiagnosticReportResult>(
                    response.result.unwrap(),
                )
                .unwrap();
                let items = match result {
                    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
                        report,
                    )) => report.full_document_diagnostic_report.items,
                    other => panic!("unexpected diagnostic response: {other:?}"),
                };
                assert!(
                    items.is_empty(),
                    "semantic diagnostics should be filtered when disabled: {items:?}"
                );
                shutdown_test_server(&client, server_thread);
                return;
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Request(request) => {
                panic!("unexpected server request during diagnostics test: {request:?}");
            }
            _ => {}
        }
    }

    panic!("documentDiagnostic response not received");
}

#[test]
fn unconfigured_workspace_reports_only_syntax_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let file_text = "\
module child(input logic a, input logic b);
endmodule

module top;
  logic sig;
  child u(.a(sig));
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(pull_caps, UserConfig::default(), file_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 1);
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("port 'b' has no connection")),
        "unconfigured workspaces should suppress semantic diagnostics: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unconfigured_workspace_diagnostics_skip_unopened_indexed_files() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("unconfigured-index-diagnostics");
    let broken_path = temp_dir.path().join("broken.sv");
    let top_path = temp_dir.path().join("top.sv");
    let top_text = "module top;\nendmodule\n";
    fs::write(&broken_path, "module broken(;\nendmodule\n").unwrap();
    fs::write(&top_path, top_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) = spawn_test_workspace(root_path, pull_caps, UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let broken_uri = to_proto::url_from_abs_path(broken_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);
    let _ = request_document_diagnostics(&client, top_uri, 1);

    let reported_uris = request_workspace_diagnostic_uris(&client, 3);
    assert!(
        !reported_uris.contains(&broken_uri),
        "workspace diagnostics should not report unopened indexed file {broken_uri}: {reported_uris:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn syntax_only_config_workspace_reports_only_syntax_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let file_text = "\
module child(input logic a, input logic b);
endmodule

module top;
  logic sig;
  child u(.a(sig));
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_syntax_only_config_diagnostics_test(pull_caps, UserConfig::default(), file_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 1);
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("port 'b' has no connection")),
        "syntax-only configs should suppress semantic diagnostics: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn empty_config_workspace_reports_only_syntax_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let file_text = "\
module child(input logic a, input logic b);
endmodule

module top;
  logic sig;
  child u(.a(sig));
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_empty_config_diagnostics_test(pull_caps, UserConfig::default(), file_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 1);
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("port 'b' has no connection")),
        "empty configs should suppress semantic diagnostics: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn syntax_only_config_workspace_reports_parse_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let file_text = "\
module top(;
endmodule
";
    let (_temp_dir, client, server_thread, uri) =
        setup_syntax_only_config_diagnostics_test(pull_caps, UserConfig::default(), file_text);

    let (_result_id, diagnostics) = request_document_diagnostics(&client, uri, 1);
    assert!(
        diagnostics.iter().any(|diag| diag.message.contains("expected")),
        "syntax-only configs should still report parse diagnostics: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn document_diagnostics_respect_disabled_source_root_policy() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut user_config = UserConfig::default();
    user_config.diagnostics.semantic.enable = false;
    let temp_dir = TempDir::new("disabled-root-document-diagnostics");
    let ignored_dir = temp_dir.path().join("ignored");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&ignored_dir).unwrap();
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "sources = [\"rtl/**\"]\nexclude = [\"ignored/**\"]\n",
    )
    .unwrap();
    let ignored_path = ignored_dir.join("ignored.sv");
    let ignored_text = "module ignored(;\nendmodule\n";
    fs::write(&ignored_path, ignored_text).unwrap();
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, "module top;\nendmodule\n").unwrap();

    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, user_config);
    let ignored_uri = to_proto::url_from_abs_path(ignored_path.as_path()).unwrap();
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    open_test_document(&client, ignored_uri.clone(), ignored_text);

    // The ignored document legitimately has no result id, just like the cold
    // fallback.  Synchronize through an enabled source first, then make one raw
    // request so the two empty responses are never conflated.
    let _ = request_document_diagnostics(&client, top_uri, 1);
    let (result_id, diagnostics) = request_document_diagnostics_once(
        &client,
        ignored_uri,
        lsp_server::RequestId::from(3),
        None,
    );
    assert!(result_id.is_none(), "disabled source roots should not receive result ids");
    assert!(
        diagnostics.is_empty(),
        "disabled source roots must not leak parse diagnostics: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn workspace_diagnostics_use_multi_file_semantic_context() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        pull_caps,
        UserConfig::default(),
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("unused.sv", "module unused;\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let unused_uri = uris[1].clone();
    let top_uri = uris[2].clone();

    let report = request_workspace_diagnostic_report_until(
        &client,
        1,
        |report| {
            report.items.iter().any(|item| {
                let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item else {
                    return false;
                };
                full.uri == top_uri
                    && full
                        .full_document_diagnostic_report
                        .items
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection"))
            })
        },
        "multi-file semantic workspace diagnostics",
    );

    let mut child_diagnostics = None;
    let mut unused_diagnostics = None;
    let mut top_diagnostics = None;
    for item in report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item {
            if full.uri == child_uri {
                child_diagnostics = Some(full.full_document_diagnostic_report.items);
            } else if full.uri == unused_uri {
                unused_diagnostics = Some(full.full_document_diagnostic_report.items);
            } else if full.uri == top_uri {
                top_diagnostics = Some(full.full_document_diagnostic_report.items);
            }
        }
    }

    let child_diagnostics = child_diagnostics.expect("missing child diagnostics");
    let unused_diagnostics = unused_diagnostics.expect("missing unused diagnostics");
    let top_diagnostics = top_diagnostics.expect("missing top diagnostics");
    assert!(
        child_diagnostics.is_empty(),
        "child.sv should not receive top.sv diagnostics: {child_diagnostics:?}"
    );
    assert!(
        unused_diagnostics.is_empty(),
        "unused.sv should not receive top.sv diagnostics: {unused_diagnostics:?}"
    );
    assert!(
        top_diagnostics.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "top.sv should receive semantic diagnostic using child.sv: {top_diagnostics:?}"
    );
    assert_eq!(
        top_diagnostics
            .iter()
            .filter(|diag| diag.message.contains("port 'b' has no connection"))
            .count(),
        1,
        "workspace diagnostics should not duplicate source-root diagnostics: {top_diagnostics:?}"
    );
    assert!(
        !top_diagnostics.iter().any(|diag| diag.message.contains("unknown module 'child'")),
        "top.sv should resolve child module from child.sv: {top_diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn workspace_diagnostics_compute_profile_owner_once_across_source_roots() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("workspace-profile-diagnostic-owner");
    let app_dir = temp_dir.path().join("app");
    let app_rtl = app_dir.join("rtl");
    let lib_dir = temp_dir.path().join("lib");
    let lib_rtl = lib_dir.join("rtl");
    fs::create_dir_all(&app_rtl).unwrap();
    fs::create_dir_all(&lib_rtl).unwrap();
    fs::write(
        app_dir.join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\nlibraries = [\"../lib\"]\n",
    )
    .unwrap();
    fs::write(lib_dir.join("vide.toml"), "sources = [\"rtl/**\"]\n").unwrap();
    fs::write(lib_rtl.join("child.sv"), "module child(input logic a, input logic b);\nendmodule\n")
        .unwrap();
    let top_path = app_rtl.join("top.sv");
    fs::write(&top_path, "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n").unwrap();

    let opt = Opt {
        process_name: "vide-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
        profile_trace: None,
    };
    let config = config::Config::new(
        opt,
        temp_dir.path().to_path_buf(),
        pull_caps,
        vec![app_dir],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();

    let report = request_workspace_diagnostic_report_until(
        &client,
        1,
        |report| {
            report
                .items
                .iter()
                .filter_map(|item| match item {
                    lsp_types::WorkspaceDocumentDiagnosticReport::Full(full)
                        if full.uri == top_uri =>
                    {
                        Some(&full.full_document_diagnostic_report.items)
                    }
                    _ => None,
                })
                .flatten()
                .filter(|diag| diag.message.contains("port 'b' has no connection"))
                .count()
                == 1
        },
        "profile semantic workspace diagnostics",
    );
    let missing_port_count = report
        .items
        .into_iter()
        .filter_map(|item| match item {
            lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) if full.uri == top_uri => {
                Some(full.full_document_diagnostic_report.items)
            }
            _ => None,
        })
        .flatten()
        .filter(|diag| diag.message.contains("port 'b' has no connection"))
        .count();

    assert_eq!(
        missing_port_count, 1,
        "profile-owned workspace diagnostics must not duplicate diagnostics per source root"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn configured_include_dirs_suppress_include_defined_macro_diagnostic() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("configured-includes");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    fs::write(include_dir.join("common_defs.svh"), "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"common_defs.svh\"\n`ifndef ENABLE_COUNTER\nmodule broken(;\nendmodule\n`endif\nmodule top;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

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
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: top_uri.clone(),
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: top_text.to_string(),
                },
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
                text_document: TextDocumentIdentifier { uri: top_uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let (_, diagnostics) = recv_document_diagnostics(&client, request_id);
    assert!(
        diagnostics.iter().all(|diag| !diag.message.contains("ENABLE_COUNTER")
            && !diag.message.contains("unknown macro")),
        "configured include_dirs should resolve include-defined macros: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unsaved_library_include_header_changes_are_used_for_dependent_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("library-include-changes");
    let app_dir = temp_dir.path().join("app");
    let app_rtl_dir = app_dir.join("rtl");
    let package_dir = temp_dir.path().join("pkg");
    let package_include_dir = package_dir.join("include");
    fs::create_dir_all(&app_rtl_dir).unwrap();
    fs::create_dir_all(&package_include_dir).unwrap();
    fs::write(
        app_dir.join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"../pkg/include\"]\nlibraries = [\"../pkg\"]\n",
    )
    .unwrap();
    fs::write(package_dir.join("vide.toml"), "sources = []\ninclude_dirs = [\"include\"]\n")
        .unwrap();

    let header_path = package_include_dir.join("defs.svh");
    fs::write(&header_path, "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = app_rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let root_path = temp_dir.path().to_path_buf();
    let app_root = app_dir.clone();
    let package_root = package_dir.clone();
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
        vec![app_root, package_root],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();

    let (_, initial_diagnostics) = request_document_diagnostics(&client, top_uri.clone(), 1);
    assert!(
        initial_diagnostics.is_empty(),
        "saved library include header should define ENABLE_COUNTER: {initial_diagnostics:?}"
    );

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: header_uri,
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: String::new(),
                },
            },
        )))
        .unwrap();

    let second_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            second_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: top_uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let (_, diagnostics_after_unsaved_header) = recv_document_diagnostics(&client, second_id);
    assert!(
        !diagnostics_after_unsaved_header.is_empty(),
        "unsaved library include header should affect dependent diagnostics: {diagnostics_after_unsaved_header:?}"
    );
    let macro_use_line =
        top_text.lines().position(|line| line.contains("ENABLE_COUNTER")).unwrap() as u32;
    assert!(
        diagnostics_after_unsaved_header.iter().any(|diag| diag.range.start.line == macro_use_line),
        "dependent diagnostic should be reported on top.sv macro use line: {diagnostics_after_unsaved_header:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn unsaved_include_header_changes_are_used_for_dependent_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("include-changes");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let header_path = include_dir.join("common_defs.svh");
    let header_text = "`define ENABLE_COUNTER 1\n";
    fs::write(&header_path, header_text).unwrap();
    let top_text = "`include \"common_defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

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
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();

    let (_, initial_diagnostics) = request_document_diagnostics(&client, top_uri.clone(), 1);
    assert!(
        initial_diagnostics.is_empty(),
        "saved include header should define ENABLE_COUNTER: {initial_diagnostics:?}"
    );

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: header_uri.clone(),
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: String::new(),
                },
            },
        )))
        .unwrap();

    let request_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: top_uri },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let (_, diagnostics) = recv_document_diagnostics(&client, request_id);
    assert!(
        diagnostics.iter().any(|diag| diag.message.contains("expected")),
        "dependent diagnostics should use unsaved include header text: {diagnostics:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn restored_project_manifest_clears_diagnostics_for_excluded_files() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("manifest-exclude-refresh");
    let manifest_path = temp_dir.path().join("vide.toml");
    let ignored_dir = temp_dir.path().join("ignored");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&ignored_dir).unwrap();
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::write(&manifest_path, DEFAULT_TEST_CONFIG).unwrap();
    fs::write(ignored_dir.join("ignored.sv"), "module ignored(;\nendmodule\n").unwrap();
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, "module top;\nendmodule\n").unwrap();

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
    let ignored_uri =
        to_proto::url_from_abs_path(ignored_dir.join("ignored.sv").as_path()).unwrap();
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let manifest_uri = to_proto::url_from_abs_path(manifest_path.as_path()).unwrap();

    let _ = request_document_diagnostics(&client, ignored_uri.clone(), 100);
    let first_report = request_workspace_diagnostic_report(&client, 1, Vec::new());
    let mut saw_ignored_diagnostic = false;
    let mut ignored_result_id = None;
    for item in first_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == ignored_uri
        {
            ignored_result_id = full.full_document_diagnostic_report.result_id.clone();
            saw_ignored_diagnostic = full
                .full_document_diagnostic_report
                .items
                .iter()
                .any(|diag| diag.message.contains("expected"));
        }
    }
    assert!(saw_ignored_diagnostic, "root-scanning config should diagnose ignored.sv");
    let ignored_result_id =
        ignored_result_id.expect("ignored.sv should have a workspace diagnostic result id");

    fs::write(
        &manifest_path,
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\nexclude = [\"ignored/**\"]\n",
    )
    .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidSaveTextDocument::METHOD.to_string(),
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: manifest_uri },
                text: None,
            },
        )))
        .unwrap();

    // The second workspace report is expected to contain a legitimate empty
    // cleanup entry.  Synchronize through the still-enabled top-level source so
    // an empty cold fallback cannot satisfy that assertion by accident.
    let _ = request_document_diagnostics(&client, top_uri, 101);
    let second_report = request_workspace_diagnostic_report(
        &client,
        2,
        vec![lsp_types::PreviousResultId { uri: ignored_uri.clone(), value: ignored_result_id }],
    );
    for item in second_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == ignored_uri
        {
            assert!(
                full.full_document_diagnostic_report.items.is_empty(),
                "restored config should clear diagnostics for excluded file: {:?}",
                full.full_document_diagnostic_report.items
            );
            shutdown_test_server(&client, server_thread);
            return;
        }
    }

    panic!("workspace diagnostics should include an empty report for previously loaded ignored.sv");
}

#[test]
fn workspace_scan_refreshes_diagnostics_for_unopened_systemverilog_dependency() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("workspace-scan");
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.v");
    let top_text = "module top;\n  wire sig;\n  child u(.a(sig));\nendmodule\n";
    fs::write(temp_dir.path().join("vide.toml"), DEFAULT_TEST_CONFIG).unwrap();
    fs::write(&child_path, "module child(input logic a, input logic b);\nendmodule\n").unwrap();
    fs::write(&top_path, top_text).unwrap();

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
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: top_uri.clone(),
                    language_id: "verilog".to_string(),
                    version: 1,
                    text: top_text.to_string(),
                },
            },
        )))
        .unwrap();

    let report = request_workspace_diagnostic_report_until(
        &client,
        1,
        |report| {
            report.items.iter().any(|item| {
                let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item else {
                    return false;
                };
                full.uri == top_uri
                    && !full
                        .full_document_diagnostic_report
                        .items
                        .iter()
                        .any(|diag| diag.message.contains("unknown module 'child'"))
                    && full
                        .full_document_diagnostic_report
                        .items
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection"))
            })
        },
        "workspace semantic diagnostics for unopened dependency",
    );
    let top_diagnostics = report
        .items
        .into_iter()
        .find_map(|item| match item {
            lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) if full.uri == top_uri => {
                Some(full.full_document_diagnostic_report.items)
            }
            _ => None,
        })
        .expect("missing top diagnostics");
    assert!(
        !top_diagnostics.iter().any(|diag| diag.message.contains("unknown module 'child'")),
        "top.v should resolve child module from unopened child.sv: {top_diagnostics:?}"
    );
    assert!(
        top_diagnostics.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "top.v should use unopened child.sv module definition: {top_diagnostics:?}"
    );
    shutdown_test_server(&client, server_thread);
}

#[test]
fn deleted_workspace_file_requests_diagnostic_refresh() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("workspace-delete-diagnostic-refresh");
    let broken_path = temp_dir.path().join("broken.sv");
    fs::write(temp_dir.path().join("vide.toml"), DEFAULT_TEST_CONFIG).unwrap();
    fs::write(&broken_path, "module broken(;\nendmodule\n").unwrap();

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
    let broken_uri = to_proto::url_from_abs_path(broken_path.as_path()).unwrap();

    let _ = request_document_diagnostics(&client, broken_uri.clone(), 100);
    let first_report = request_workspace_diagnostic_report(&client, 1, Vec::new());
    let mut saw_broken_diagnostic = false;
    let mut broken_result_id = None;
    for item in first_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == broken_uri
        {
            broken_result_id = full.full_document_diagnostic_report.result_id.clone();
            saw_broken_diagnostic = full
                .full_document_diagnostic_report
                .items
                .iter()
                .any(|diag| diag.message.contains("expected"));
        }
    }
    assert!(saw_broken_diagnostic, "expected broken.sv diagnostics before deletion");
    let broken_result_id =
        broken_result_id.expect("workspace diagnostics need result ids to clear deleted files");

    fs::remove_file(&broken_path).unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeWatchedFiles::METHOD.to_string(),
            lsp_types::DidChangeWatchedFilesParams {
                changes: vec![FileEvent::new(broken_uri.clone(), FileChangeType::DELETED)],
            },
        )))
        .unwrap();

    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    let mut saw_refresh = false;
    while let Some(message) =
        recv_lsp_message_until(&client, deadline, "workspace diagnostic refresh")
    {
        match message {
            Message::Request(request)
                if request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
                saw_refresh = true;
                break;
            }
            Message::Request(request)
                if request.method == lsp_types::request::WorkDoneProgressCreate::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            other => panic!("unexpected message while waiting for diagnostic refresh: {other:?}"),
        }
    }
    assert!(saw_refresh, "deleting a diagnosed workspace file should refresh pulled diagnostics");

    let second_report = request_workspace_diagnostic_report(
        &client,
        2,
        vec![lsp_types::PreviousResultId { uri: broken_uri.clone(), value: broken_result_id }],
    );
    for item in second_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == broken_uri
        {
            assert!(
                full.full_document_diagnostic_report.items.is_empty(),
                "deleted file diagnostics should be cleared: {:?}",
                full.full_document_diagnostic_report.items
            );
            shutdown_test_server(&client, server_thread);
            return;
        }
    }

    panic!("workspace diagnostics should include an empty report for deleted broken.sv");
}

#[test]
fn watched_dependency_change_refreshes_workspace_diagnostics() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            diagnostic: Some(lsp_types::DiagnosticWorkspaceClientCapabilities {
                refresh_support: Some(true),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("workspace-watcher-diagnostic-refresh");
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.sv");
    fs::write(temp_dir.path().join("vide.toml"), DEFAULT_TEST_CONFIG).unwrap();
    fs::write(&child_path, "module child(input logic a, input logic b);\nendmodule\n").unwrap();
    fs::write(&top_path, "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n").unwrap();

    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, UserConfig::default());
    let child_uri = to_proto::url_from_abs_path(child_path.as_path()).unwrap();
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();

    let first_report = request_workspace_diagnostic_report_until(
        &client,
        1,
        |report| {
            report.items.iter().any(|item| {
                let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item else {
                    return false;
                };
                full.uri == top_uri
                    && full
                        .full_document_diagnostic_report
                        .items
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection"))
            })
        },
        "initial watched dependency semantic diagnostics",
    );
    let mut saw_missing_port = false;
    for item in first_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == top_uri
        {
            saw_missing_port = full
                .full_document_diagnostic_report
                .items
                .iter()
                .any(|diag| diag.message.contains("port 'b' has no connection"));
        }
    }
    assert!(saw_missing_port, "expected top.sv missing port diagnostic before dependency edit");

    fs::write(&child_path, "module child(input logic a);\nendmodule\n").unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeWatchedFiles::METHOD.to_string(),
            lsp_types::DidChangeWatchedFilesParams {
                changes: vec![FileEvent::new(child_uri, FileChangeType::CHANGED)],
            },
        )))
        .unwrap();

    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    let mut saw_refresh = false;
    while let Some(message) =
        recv_lsp_message_until(&client, deadline, "workspace diagnostic refresh")
    {
        match message {
            Message::Request(request)
                if request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
                saw_refresh = true;
                break;
            }
            Message::Request(request)
                if request.method == lsp_types::request::WorkDoneProgressCreate::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            other => panic!("unexpected message while waiting for diagnostic refresh: {other:?}"),
        }
    }
    assert!(saw_refresh, "changing a watched dependency should refresh pulled diagnostics");

    let second_report = request_workspace_diagnostic_report(&client, 2, Vec::new());
    for item in second_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == top_uri
        {
            assert!(
                full.full_document_diagnostic_report.items.is_empty(),
                "top.sv diagnostics should refresh after watched dependency edit: {:?}",
                full.full_document_diagnostic_report.items
            );
            shutdown_test_server(&client, server_thread);
            return;
        }
    }

    panic!("workspace diagnostics should include top.sv after watched dependency edit");
}

#[test]
fn document_diagnostic_result_id_tracks_diagnostics_config_revision() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_temp_dir, client, server_thread, uri) = setup_configured_diagnostics_test(
        pull_caps,
        UserConfig::default(),
        "module top;\nendmodule\n",
    );

    let (first_result_id, first_items) = request_document_diagnostics_until(
        &client,
        uri.clone(),
        1,
        |result_id, items| {
            items.is_empty()
                && result_id.is_some_and(|result_id| result_id.contains("external-slang-semantic"))
        },
        "initial empty semantic diagnostics result id",
    );
    let first_result_id = first_result_id.expect("expected first diagnostic result id");
    assert!(
        first_items.is_empty(),
        "test fixture should start without diagnostics: {first_items:?}"
    );

    let (second_result_id, second_items) = request_document_diagnostics_with_previous_result_id(
        &client,
        uri.clone(),
        9,
        Some(first_result_id.clone()),
    );
    assert_eq!(
        second_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "unchanged diagnostics config must keep the previous result id"
    );
    assert!(
        second_items.is_empty(),
        "unchanged diagnostic reports should not resend items: {second_items:?}"
    );

    update_test_configuration(
        &client,
        serde_json::json!({
            "diagnostics": {
                "semantic": { "enable": true }
            }
        }),
    );

    let (same_config_result_id, same_config_items) =
        request_document_diagnostics_with_previous_result_id(
            &client,
            uri.clone(),
            10,
            Some(first_result_id.clone()),
        );
    assert_eq!(
        same_config_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "configuration refreshes without diagnostic config changes must keep the previous result id"
    );
    assert!(
        same_config_items.is_empty(),
        "unchanged diagnostic reports should not resend items: {same_config_items:?}"
    );

    update_test_configuration(
        &client,
        serde_json::json!({
            "diagnostics": {
                "semantic": { "enable": false }
            }
        }),
    );

    let (third_result_id, _third_items) = request_document_diagnostics_with_previous_result_id(
        &client,
        uri,
        11,
        Some(first_result_id),
    );
    assert_ne!(
        third_result_id.as_deref(),
        second_result_id.as_deref(),
        "diagnostics config changes must invalidate the result id"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn document_diagnostic_result_id_changes_when_dependency_changes() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        pull_caps,
        UserConfig::default(),
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();

    let (first_result_id, first_items) = request_document_diagnostics_until(
        &client,
        top_uri.clone(),
        1,
        |_result_id, items| {
            items.iter().any(|diag| diag.message.contains("port 'b' has no connection"))
        },
        "initial dependency semantic diagnostics",
    );
    let first_result_id = first_result_id.expect("expected first diagnostic result id");
    assert!(!first_result_id.is_empty(), "diagnostic result id should include open file versions");
    assert!(
        first_items.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "expected missing port diagnostic before dependency edit: {first_items:?}"
    );

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: child_uri, version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "module child(input logic a);\nendmodule\n".to_string(),
                }],
            },
        )))
        .unwrap();

    let second_id = lsp_server::RequestId::from(9);
    client
        .sender
        .send(Message::Request(Request::new(
            second_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: top_uri },
                identifier: None,
                previous_result_id: Some(first_result_id.clone()),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let (second_result_id, second_items) = recv_document_diagnostics(&client, second_id);
    assert_ne!(
        second_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "dependency edit must invalidate top.sv diagnostic result id"
    );
    assert!(
        second_items.is_empty(),
        "missing port diagnostic should disappear after dependency edit: {second_items:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn document_diagnostic_result_id_changes_when_include_dependency_changes() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("diagnostic-result-id-include-dependency");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let header_path = include_dir.join("defs.svh");
    fs::write(&header_path, "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, UserConfig::default());
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();

    let (first_result_id, first_items) = request_document_diagnostics(&client, top_uri.clone(), 1);
    let first_result_id = first_result_id.expect("expected first diagnostic result id");
    assert!(first_items.is_empty(), "fixture should start clean: {first_items:?}");

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: header_uri,
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: String::new(),
                },
            },
        )))
        .unwrap();

    let (second_result_id, second_items) = request_document_diagnostics_with_previous_result_id(
        &client,
        top_uri,
        2,
        Some(first_result_id.clone()),
    );
    assert_ne!(
        second_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "include dependency edits must invalidate the dependent document result id"
    );
    assert!(
        second_items.iter().any(|diag| diag.message.contains("expected")),
        "dependent diagnostics should be recomputed from changed include text: {second_items:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn syntax_only_document_result_id_changes_when_include_dependency_changes() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut user_config = UserConfig::default();
    user_config.diagnostics.semantic.enable = false;
    let temp_dir = TempDir::new("syntax-result-id-include-dependency");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let header_path = include_dir.join("defs.svh");
    fs::write(&header_path, "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, user_config);
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();

    let (first_result_id, first_items) = request_document_diagnostics(&client, top_uri.clone(), 1);
    let first_result_id = first_result_id.expect("expected first diagnostic result id");
    assert!(first_items.is_empty(), "fixture should start clean: {first_items:?}");

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: header_uri,
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: String::new(),
                },
            },
        )))
        .unwrap();

    let (second_result_id, second_items) = request_document_diagnostics_with_previous_result_id(
        &client,
        top_uri,
        2,
        Some(first_result_id.clone()),
    );
    assert_ne!(
        second_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "syntax-only include dependency edits must invalidate the dependent result id"
    );
    assert!(
        second_items.iter().any(|diag| diag.message.contains("expected")),
        "syntax-only diagnostics should be recomputed from changed include text: {second_items:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn document_diagnostic_result_id_ignores_unrelated_profile_changes() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new("diagnostic-result-id-profile-scope");
    let app_a_dir = temp_dir.path().join("app_a");
    let app_b_dir = temp_dir.path().join("app_b");
    let app_a_rtl = app_a_dir.join("rtl");
    let app_b_rtl = app_b_dir.join("rtl");
    fs::create_dir_all(&app_a_rtl).unwrap();
    fs::create_dir_all(&app_b_rtl).unwrap();
    fs::write(
        app_a_dir.join("vide.toml"),
        "top_modules = [\"top_a\"]\nsources = [\"rtl/**\"]\ninclude_dirs = []\n",
    )
    .unwrap();
    fs::write(
        app_b_dir.join("vide.toml"),
        "top_modules = [\"top_b\"]\nsources = [\"rtl/**\"]\ninclude_dirs = []\n",
    )
    .unwrap();
    let app_a_text = "module top_a;\nendmodule\n";
    let app_b_text = "module top_b;\nendmodule\n";
    let app_a_path = app_a_rtl.join("top_a.sv");
    let app_b_path = app_b_rtl.join("top_b.sv");
    fs::write(&app_a_path, app_a_text).unwrap();
    fs::write(&app_b_path, app_b_text).unwrap();

    let (client, server_thread) =
        spawn_test_workspace(temp_dir.path().to_path_buf(), pull_caps, UserConfig::default());
    let app_a_uri = to_proto::url_from_abs_path(app_a_path.as_path()).unwrap();
    let app_b_uri = to_proto::url_from_abs_path(app_b_path.as_path()).unwrap();
    open_test_document(&client, app_a_uri.clone(), app_a_text);
    open_test_document(&client, app_b_uri.clone(), app_b_text);

    let (first_result_id, first_items) =
        request_document_diagnostics(&client, app_a_uri.clone(), 1);
    let first_result_id = first_result_id.expect("expected first diagnostic result id");
    assert!(first_items.is_empty(), "fixture should start clean: {first_items:?}");

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: app_b_uri, version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "module top_b;\n  logic unrelated;\nendmodule\n".to_string(),
                }],
            },
        )))
        .unwrap();

    let (second_result_id, second_items) = request_document_diagnostics_with_previous_result_id(
        &client,
        app_a_uri,
        2,
        Some(first_result_id.clone()),
    );
    assert_eq!(
        second_result_id.as_deref(),
        Some(first_result_id.as_str()),
        "edits in a different semantic profile must not invalidate this result id"
    );
    assert!(
        second_items.is_empty(),
        "unchanged diagnostic reports should not resend items: {second_items:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn legacy_publish_diagnostics_refreshes_dependent_open_files() {
    let mut user_config = UserConfig::default();
    user_config.diagnostics.update = DiagnosticsUpdateUserConfig::OnType;

    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        user_config,
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();

    let first_top_diags = recv_publish_diagnostics_for_uri(&client, &top_uri);
    assert!(
        first_top_diags.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "expected initial top.sv missing port diagnostic: {first_top_diags:?}"
    );

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier { uri: child_uri, version: 2 },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "module child(input logic a);\nendmodule\n".to_string(),
                }],
            },
        )))
        .unwrap();

    let second_top_diags = recv_empty_publish_diagnostics_for_uri(
        &client,
        &top_uri,
        "dependent diagnostics after child.sv edit",
    );
    assert!(
        second_top_diags.is_empty(),
        "top.sv diagnostics should refresh when child.sv changes: {second_top_diags:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn legacy_on_save_diagnostics_refresh_profile_dependents() {
    let mut user_config = UserConfig::default();
    user_config.diagnostics.update = DiagnosticsUpdateUserConfig::OnSave;

    let (_temp_dir, client, server_thread, uris) = setup_configured_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        user_config,
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();

    let first_top_diags = recv_publish_diagnostics_for_uri(&client, &top_uri);
    assert!(
        first_top_diags.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "expected initial top.sv missing port diagnostic: {first_top_diags:?}"
    );

    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeTextDocument::METHOD.to_string(),
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: child_uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "module child(input logic a);\nendmodule\n".to_string(),
                }],
            },
        )))
        .unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidSaveTextDocument::METHOD.to_string(),
            DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: child_uri },
                text: None,
            },
        )))
        .unwrap();

    let second_top_diags = recv_empty_publish_diagnostics_for_uri(
        &client,
        &top_uri,
        "dependent diagnostics after child.sv save",
    );
    assert!(
        second_top_diags.is_empty(),
        "saving child.sv should refresh dependent top.sv diagnostics: {second_top_diags:?}"
    );

    shutdown_test_server(&client, server_thread);
}

#[test]
fn legacy_on_save_watched_include_refreshes_profile_dependents() {
    let mut user_config = UserConfig::default();
    user_config.diagnostics.update = DiagnosticsUpdateUserConfig::OnSave;

    let temp_dir = TempDir::new("legacy-watched-include-diagnostics");
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vide.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl/**\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let header_path = include_dir.join("ports.svh");
    fs::write(&header_path, "`define CHILD_PORTS input logic a, input logic b\n").unwrap();
    fs::write(
        rtl_dir.join("child.sv"),
        "`include \"ports.svh\"\nmodule child(`CHILD_PORTS);\nendmodule\n",
    )
    .unwrap();
    let top_path = rtl_dir.join("top.sv");
    let top_text = "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n";
    fs::write(&top_path, top_text).unwrap();

    let (client, server_thread) = spawn_test_workspace(
        temp_dir.path().to_path_buf(),
        ClientCapabilities::default(),
        user_config,
    );
    let top_uri = to_proto::url_from_abs_path(top_path.as_path()).unwrap();
    let header_uri = to_proto::url_from_abs_path(header_path.as_path()).unwrap();
    open_test_document(&client, top_uri.clone(), top_text);

    let first_top_diags = recv_publish_diagnostics_for_uri(&client, &top_uri);
    assert!(
        first_top_diags.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
        "expected initial top.sv missing port diagnostic: {first_top_diags:?}"
    );

    fs::write(&header_path, "`define CHILD_PORTS input logic a\n").unwrap();
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeWatchedFiles::METHOD.to_string(),
            lsp_types::DidChangeWatchedFilesParams {
                changes: vec![FileEvent::new(header_uri, FileChangeType::CHANGED)],
            },
        )))
        .unwrap();

    let second_top_diags = recv_empty_publish_diagnostics_for_uri(
        &client,
        &top_uri,
        "dependent diagnostics after watched include change",
    );
    assert!(
        second_top_diags.is_empty(),
        "watched include changes should refresh dependent top.sv diagnostics: {second_top_diags:?}"
    );

    shutdown_test_server(&client, server_thread);
}
