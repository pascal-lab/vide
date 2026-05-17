use std::{
    env, fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use lsp_server::{Connection, Message, Notification, Request};
use lsp_types::{
    ClientCapabilities, CodeActionCapabilityResolveSupport, CodeActionClientCapabilities,
    CodeActionContext, CodeActionKind, CodeActionKindLiteralSupport, CodeActionLiteralSupport,
    CodeActionOrCommand, CodeActionParams, DiagnosticClientCapabilities,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, Position, ProgressParams,
    PublishDiagnosticsParams, Range, SemanticTokensParams, SemanticTokensResult,
    TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Url, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams, WorkspaceClientCapabilities, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReportResult,
    notification::{
        DidChangeTextDocument, DidOpenTextDocument, DidSaveTextDocument, Exit, Notification as _,
    },
    request::{
        CodeActionRequest, DocumentDiagnosticRequest, DocumentSymbolRequest, FoldingRangeRequest,
        GotoDefinition, HoverRequest, Request as _, SemanticTokensFullRequest, Shutdown,
        WorkspaceDiagnosticRequest,
    },
};
use serde::de::DeserializeOwned;
use utils::paths::AbsPathBuf;

use crate::{
    Opt,
    config::{
        self,
        user_config::{DiagnosticsUpdateUserConfig, UserConfig},
    },
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

    let uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(file_path.clone()).as_ref()).unwrap();
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
        uris.push(
            to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(file_path.clone()).as_ref())
                .unwrap(),
        );
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

fn recv_response<T: DeserializeOwned>(
    client: &Connection,
    request_id: lsp_server::RequestId,
    label: &str,
) -> T {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Response(response) if response.id == request_id => {
                assert!(response.error.is_none(), "{label} returned error: {:?}", response.error);
                return serde_json::from_value(response.result.unwrap_or(serde_json::Value::Null))
                    .unwrap_or_else(|err| panic!("failed to decode {label} response: {err}"));
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            Message::Request(request)
                if request.method == lsp_types::request::WorkDoneProgressCreate::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
            }
            Message::Request(request)
                if request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
            }
            Message::Request(request) => {
                panic!("unexpected server request during {label}: {request:?}");
            }
            _ => {}
        }
    }

    panic!("{label} response not received");
}

fn position_of(text: &str, needle: &str) -> Position {
    let offset = text.find(needle).unwrap_or_else(|| panic!("missing {needle:?}"));
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    Position { line, character: (offset - line_start) as u32 }
}

fn code_action_client_caps() -> ClientCapabilities {
    ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            code_action: Some(CodeActionClientCapabilities {
                code_action_literal_support: Some(CodeActionLiteralSupport {
                    code_action_kind: CodeActionKindLiteralSupport {
                        value_set: [
                            CodeActionKind::EMPTY,
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::REFACTOR,
                            CodeActionKind::REFACTOR_REWRITE,
                        ]
                        .into_iter()
                        .map(|kind| kind.as_str().to_owned())
                        .collect(),
                    },
                }),
                resolve_support: Some(CodeActionCapabilityResolveSupport {
                    properties: vec!["edit".to_owned()],
                }),
                ..Default::default()
            }),
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn request_code_actions(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    context: CodeActionContext,
    request_id: i32,
) -> Vec<CodeActionOrCommand> {
    let position = position_of(text, needle);
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            CodeActionRequest::METHOD.to_string(),
            CodeActionParams {
                text_document: TextDocumentIdentifier { uri },
                range: Range::new(position, position),
                context,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    recv_response(client, request_id, "codeAction")
}

fn code_action_titles(actions: &[CodeActionOrCommand]) -> Vec<String> {
    actions
        .iter()
        .map(|action| match action {
            CodeActionOrCommand::CodeAction(action) => action.title.clone(),
            CodeActionOrCommand::Command(command) => command.title.clone(),
        })
        .collect()
}

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
        setup_diagnostics_test(code_action_client_caps(), UserConfig::default(), text);

    let diagnostics_id = lsp_server::RequestId::from(210);
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
    let (_result_id, mut diagnostics) = recv_document_diagnostics(&client, diagnostics_id);
    assert!(!diagnostics.is_empty(), "expected mixed connection diagnostic");
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
                .all(|diag| diag.source.as_deref() != Some("vizsla")),
            "document diagnostics should not include removed Vizsla model diagnostics"
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
            ("unused.sv", "module unused;\nendmodule\n"),
            ("top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ],
    );
    let child_uri = uris[0].clone();
    let unused_uri = uris[1].clone();
    let top_uri = uris[2].clone();

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
                    top_diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection")),
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
fn configured_include_dirs_suppress_include_defined_macro_diagnostic() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new();
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vizsla_config.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    fs::write(include_dir.join("common_defs.svh"), "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"common_defs.svh\"\n`ifndef ENABLE_COUNTER\nmodule broken(;\nendmodule\n`endif\nmodule top;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let top_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(top_path.clone()).as_ref()).unwrap();
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
    let temp_dir = TempDir::new();
    let app_dir = temp_dir.path().join("app");
    let app_rtl_dir = app_dir.join("rtl");
    let package_dir = temp_dir.path().join("pkg");
    let package_include_dir = package_dir.join("include");
    fs::create_dir_all(&app_rtl_dir).unwrap();
    fs::create_dir_all(&package_include_dir).unwrap();
    fs::write(
        app_dir.join("vizsla_config.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl\"]\ninclude_dirs = [\"../pkg/include\"]\nlibraries = [\"../pkg\"]\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("vizsla_config.toml"),
        "sources = []\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();

    let header_path = package_include_dir.join("defs.svh");
    fs::write(&header_path, "`define ENABLE_COUNTER 1\n").unwrap();
    let top_text = "`include \"defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = app_rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let app_root = AbsPathBuf::assert_utf8(app_dir.clone());
    let package_root = AbsPathBuf::assert_utf8(package_dir.clone());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![app_root, package_root],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let top_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(top_path.clone()).as_ref()).unwrap();
    let header_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(header_path.clone()).as_ref()).unwrap();

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
    let (_, initial_diagnostics) = recv_document_diagnostics(&client, first_id);
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
    let temp_dir = TempDir::new();
    let rtl_dir = temp_dir.path().join("rtl");
    let include_dir = temp_dir.path().join("include");
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::create_dir_all(&include_dir).unwrap();
    fs::write(
        temp_dir.path().join("vizsla_config.toml"),
        "top_modules = [\"top\"]\nsources = [\"rtl\"]\ninclude_dirs = [\"include\"]\n",
    )
    .unwrap();
    let header_path = include_dir.join("common_defs.svh");
    let header_text = "`define ENABLE_COUNTER 1\n";
    fs::write(&header_path, header_text).unwrap();
    let top_text = "`include \"common_defs.svh\"\nmodule top;\n  logic enable;\n  always_comb enable = `ENABLE_COUNTER;\nendmodule\n";
    let top_path = rtl_dir.join("top.sv");
    fs::write(&top_path, top_text).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let top_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(top_path.clone()).as_ref()).unwrap();
    let header_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(header_path.clone()).as_ref()).unwrap();

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
    let (_, initial_diagnostics) = recv_document_diagnostics(&client, first_id);
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
fn project_manifest_is_not_diagnosed_as_systemverilog() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new();
    let manifest_text = "top_modules = [\"top\"]\nsources = [\"rtl\"]\n";
    let manifest_path = temp_dir.path().join("vizsla_config.toml");
    fs::write(&manifest_path, manifest_text).unwrap();
    fs::create_dir_all(temp_dir.path().join("rtl")).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let manifest_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(manifest_path.clone()).as_ref())
            .unwrap();

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

#[test]
fn restored_project_manifest_clears_diagnostics_for_excluded_files() {
    let pull_caps = ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let temp_dir = TempDir::new();
    let manifest_path = temp_dir.path().join("vizsla_config.toml");
    let ignored_dir = temp_dir.path().join("ignored");
    let rtl_dir = temp_dir.path().join("rtl");
    fs::create_dir_all(&ignored_dir).unwrap();
    fs::create_dir_all(&rtl_dir).unwrap();
    fs::write(&manifest_path, "").unwrap();
    fs::write(ignored_dir.join("ignored.sv"), "module ignored(;\nendmodule\n").unwrap();
    fs::write(rtl_dir.join("top.sv"), "module top;\nendmodule\n").unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let ignored_uri = to_proto::url_from_abs_path(
        AbsPathBuf::assert_utf8(ignored_dir.join("ignored.sv")).as_ref(),
    )
    .unwrap();
    let manifest_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(manifest_path.clone()).as_ref())
            .unwrap();

    let first_id = lsp_server::RequestId::from(1);
    client
        .sender
        .send(Message::Request(Request::new(
            first_id.clone(),
            WorkspaceDiagnosticRequest::METHOD.to_string(),
            WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: Vec::new(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let first: WorkspaceDiagnosticReportResult =
        recv_response(&client, first_id, "workspaceDiagnostic");
    let first_report = match first {
        WorkspaceDiagnosticReportResult::Report(report) => report,
        other => panic!("unexpected workspace diagnostic response: {other:?}"),
    };
    let mut saw_ignored_diagnostic = false;
    for item in first_report.items {
        if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
            && full.uri == ignored_uri
        {
            saw_ignored_diagnostic = full
                .full_document_diagnostic_report
                .items
                .iter()
                .any(|diag| diag.message.contains("expected"));
        }
    }
    assert!(saw_ignored_diagnostic, "empty config should diagnose ignored.sv");

    fs::write(
        &manifest_path,
        "top_modules = [\"top\"]\nsources = [\"rtl\"]\nexclude = [\"ignored\"]\n",
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

    let second_id = lsp_server::RequestId::from(2);
    client
        .sender
        .send(Message::Request(Request::new(
            second_id.clone(),
            WorkspaceDiagnosticRequest::METHOD.to_string(),
            WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: Vec::new(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    let second: WorkspaceDiagnosticReportResult =
        recv_response(&client, second_id, "workspaceDiagnostic");
    let second_report = match second {
        WorkspaceDiagnosticReportResult::Report(report) => report,
        other => panic!("unexpected workspace diagnostic response: {other:?}"),
    };
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
    let temp_dir = TempDir::new();
    let child_path = temp_dir.path().join("child.sv");
    let top_path = temp_dir.path().join("top.v");
    let top_text = "module top;\n  wire sig;\n  child u(.a(sig));\nendmodule\n";
    fs::write(&child_path, "module child(input logic a, input logic b);\nendmodule\n").unwrap();
    fs::write(&top_path, top_text).unwrap();

    let root_path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
    let opt = Opt {
        process_name: "vizsla-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
    };
    let config = config::Config::new(
        opt,
        root_path.clone(),
        pull_caps,
        vec![root_path],
        UserConfig::default(),
        Vec::new(),
    );

    let (server, client) = Connection::memory();
    let server_thread = thread::spawn(move || main_loop::main_loop(config, server));
    let top_uri =
        to_proto::url_from_abs_path(AbsPathBuf::assert_utf8(top_path.clone()).as_ref()).unwrap();
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

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        match client.receiver.recv_timeout(timeout).unwrap() {
            Message::Request(request)
                if request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
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
            Message::Request(request) => {
                panic!(
                    "unexpected server request while waiting for diagnostic refresh: {request:?}"
                );
            }
            _ => {}
        }
    }

    let request_id = lsp_server::RequestId::from(1);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            WorkspaceDiagnosticRequest::METHOD.to_string(),
            WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: Vec::new(),
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

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
                let mut top_diagnostics = None;
                for item in report.items {
                    if let lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) = item
                        && full.uri == top_uri
                    {
                        top_diagnostics = Some(full.full_document_diagnostic_report.items);
                    }
                }
                let top_diagnostics = top_diagnostics.expect("missing top diagnostics");
                assert!(
                    !top_diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("unknown module 'child'")),
                    "top.v should resolve child module from unopened child.sv: {top_diagnostics:?}"
                );
                assert!(
                    top_diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("port 'b' has no connection")),
                    "top.v should use unopened child.sv module definition: {top_diagnostics:?}"
                );
                shutdown_test_server(&client, server_thread);
                return;
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Request(request)
                if request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD =>
            {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
                    .unwrap();
            }
            Message::Request(request) => {
                panic!("unexpected server request during workspace diagnostics: {request:?}");
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
    let mut user_config = UserConfig::default();
    user_config.diagnostics.update = DiagnosticsUpdateUserConfig::OnType;

    let (_temp_dir, client, server_thread, uris) = setup_multi_file_diagnostics_test(
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

    let second_top_diags = recv_publish_diagnostics_for_uri(&client, &top_uri);
    assert!(
        second_top_diags.is_empty(),
        "top.sv diagnostics should refresh when child.sv changes: {second_top_diags:?}"
    );

    shutdown_test_server(&client, server_thread);
}
