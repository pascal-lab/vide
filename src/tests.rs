use std::{
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lsp_server::{Connection, Message, Notification, Request};
use lsp_types::{
    ClientCapabilities, DiagnosticClientCapabilities, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, ProgressParams, PublishDiagnosticsParams,
    TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, Url, VersionedTextDocumentIdentifier, WorkDoneProgressParams,
    WorkspaceDiagnosticParams, WorkspaceDiagnosticReportResult,
    notification::{DidChangeTextDocument, DidOpenTextDocument, Exit, Notification as _},
    request::{DocumentDiagnosticRequest, Request as _, Shutdown, WorkspaceDiagnosticRequest},
};
use utils::paths::AbsPathBuf;

use crate::{
    Opt,
    config::{self, user_config::UserConfig},
    global_state::main_loop,
    lsp_ext::to_proto,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let unique = format!(
            "vizsla-diag-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn setup_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    let temp_dir = TempDir::new();
    let file_path = temp_dir.path().join("broken.sv");
    fs::write(&file_path, file_text).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        client_caps,
        vec![root_path],
        user_config,
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));

    let uri = to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(file_path.clone()).as_ref());
    let did_open = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "systemverilog".to_string(),
            version: 1,
            text: file_text.to_owned(),
        },
    };
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            did_open,
        )))
        .unwrap();

    (temp_dir, client, server_thread, uri)
}

fn setup_multi_file_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    files: &[(&str, &str)],
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Vec<Url>) {
    let temp_dir = TempDir::new();
    let mut uris = Vec::new();

    for (path, text) in files {
        let file_path = temp_dir.path().join(path);
        fs::write(&file_path, text).unwrap();
        uris.push(to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(file_path.clone()).as_ref()));
    }

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        client_caps,
        vec![root_path],
        user_config,
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));

    for ((path, text), uri) in files.iter().zip(uris.iter()) {
        let did_open = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "systemverilog".to_string(),
                version: 1,
                text: (*text).to_owned(),
            },
        };
        client
            .sender
            .send(Message::Notification(Notification::new(
                DidOpenTextDocument::METHOD.to_string(),
                did_open,
            )))
            .unwrap_or_else(|_| panic!("failed to open {path}"));
    }

    (temp_dir, client, server_thread, uris)
}

fn shutdown_test_server(
    client: &Connection,
    server_thread: thread::JoinHandle<anyhow::Result<()>>,
) {
    let shutdown_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(shutdown_id.clone(), Shutdown::METHOD.to_string(), ())))
        .unwrap();

    loop {
        match client.receiver.recv_timeout(Duration::from_secs(10)).unwrap() {
            Message::Response(response) if response.id == shutdown_id => break,
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            other => panic!("unexpected message while shutting down test server: {other:?}"),
        }
    }

    client
        .sender
        .send(Message::Notification(Notification::new(Exit::METHOD.to_string(), ())))
        .unwrap();

    server_thread.join().unwrap().unwrap();
}

fn recv_document_diagnostics(
    client: &Connection,
    request_id: lsp_server::RequestId,
) -> (Option<String>, Vec<lsp_types::Diagnostic>) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Response(response) if response.id == request_id => {
                assert!(response.error.is_none(), "{:?}", response.error);
                let result = serde_json::from_value::<DocumentDiagnosticReportResult>(
                    response.result.unwrap(),
                )
                .unwrap();
                return match result {
                    DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(
                        report,
                    )) => (
                        report.full_document_diagnostic_report.result_id,
                        report.full_document_diagnostic_report.items,
                    ),
                    DocumentDiagnosticReportResult::Report(
                        DocumentDiagnosticReport::Unchanged(report),
                    ) => (Some(report.unchanged_document_diagnostic_report.result_id), Vec::new()),
                    other => panic!("unexpected diagnostic response: {other:?}"),
                };
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

fn recv_publish_diagnostics_for_uri(client: &Connection, uri: &Url) -> Vec<lsp_types::Diagnostic> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD =>
            {
                let params =
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap();
                if &params.uri == uri {
                    return params.diagnostics;
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

    panic!("publishDiagnostics notification not received for {uri}");
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
    let (_temp_dir, client, server_thread, uri) =
        setup_diagnostics_test(pull_caps, UserConfig::default(), "module broken(;\nendmodule\n");
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
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    while std::time::Instant::now() < deadline && pull_diagnostics.is_none() {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
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

    let quiet_until = std::time::Instant::now() + Duration::from_millis(500);
    while std::time::Instant::now() < quiet_until {
        let timeout = quiet_until.saturating_duration_since(std::time::Instant::now());
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
    let deadline = std::time::Instant::now() + Duration::from_secs(10);

    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
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
        setup_diagnostics_test(pull_caps, user_config, file_text);

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

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
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
fn workspace_diagnostics_use_multi_file_semantic_context() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let (_temp_dir, client, server_thread, uris) = setup_multi_file_diagnostics_test(
        pull_caps,
        UserConfig::default(),
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();

    let request_id = lsp_server::RequestId::from(1);
    let request = Request::new(
        request_id.clone(),
        WorkspaceDiagnosticRequest::METHOD.to_string(),
        WorkspaceDiagnosticParams {
            identifier: None,
            previous_result_ids: Vec::new(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: Default::default(),
        },
    );
    client.sender.send(Message::Request(request)).unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Response(response) if response.id == request_id => {
                assert!(response.error.is_none(), "{:?}", response.error);
                let result = serde_json::from_value::<WorkspaceDiagnosticReportResult>(
                    response.result.unwrap(),
                )
                .unwrap();
                let report = match result {
                    WorkspaceDiagnosticReportResult::Report(report) => report,
                    other => panic!("unexpected workspace diagnostic response: {other:?}"),
                };
                let mut child_diagnostics = None;
                let mut top_diagnostics = None;
                for item in report.items {
                    if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item {
                        if full.uri == child_uri {
                            child_diagnostics = Some(full.full_document_diagnostic_report.items);
                        } else if full.uri == top_uri {
                            top_diagnostics = Some(full.full_document_diagnostic_report.items);
                        }
                    }
                }

                let child_diagnostics = child_diagnostics.expect("missing child diagnostics");
                let top_diagnostics = top_diagnostics.expect("missing top diagnostics");
                assert!(
                    child_diagnostics.is_empty(),
                    "child.sv should not receive top.sv diagnostics: {child_diagnostics:?}"
                );
                assert!(
                    top_diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection")),
                    "top.sv should receive semantic diagnostic using child.sv: {top_diagnostics:?}"
                );
                assert!(
                    !top_diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("unknown module 'child'")),
                    "top.sv should resolve child module from child.sv: {top_diagnostics:?}"
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

    panic!("workspaceDiagnostic response not received");
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
    let (_temp_dir, client, server_thread, uris) = setup_multi_file_diagnostics_test(
        pull_caps,
        UserConfig::default(),
        &[
            ("child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let top_uri = uris[1].clone();

    let first_id = lsp_server::RequestId::from(1);
    client
        .sender
        .send(Message::Request(Request::new(
            first_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: top_uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let (first_result_id, first_items) = recv_document_diagnostics(&client, first_id);
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

    let second_id = lsp_server::RequestId::from(2);
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
fn legacy_publish_diagnostics_refreshes_dependent_open_files() {
    let (_temp_dir, client, server_thread, uris) = setup_multi_file_diagnostics_test(
        ClientCapabilities::default(),
        UserConfig::default(),
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

    let second_top_diags = recv_publish_diagnostics_for_uri(&client, &top_uri);
    assert!(
        second_top_diags.is_empty(),
        "top.sv diagnostics should refresh when child.sv changes: {second_top_diags:?}"
    );

    shutdown_test_server(&client, server_thread);
}
