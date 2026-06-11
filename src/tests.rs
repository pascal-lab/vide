use std::{
    fs, thread,
    time::{Duration, Instant},
};

use lsp_server::{Connection, Message, Notification, Request};
use lsp_types::{
    ClientCapabilities, CodeActionCapabilityResolveSupport, CodeActionClientCapabilities,
    CodeActionContext, CodeActionKind, CodeActionKindLiteralSupport, CodeActionLiteralSupport,
    CodeActionOrCommand, CodeActionParams, CompletionParams, CompletionResponse,
    DiagnosticClientCapabilities, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, DocumentSymbolParams,
    DocumentSymbolResponse, FileChangeType, FileEvent, FoldingRange, FoldingRangeParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, Position, ProgressParams,
    PublishDiagnosticsParams, Range, SemanticTokensParams, SemanticTokensResult,
    TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, Url, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams, WorkspaceClientCapabilities, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReportResult, WorkspaceSymbolParams, WorkspaceSymbolResponse,
    notification::{
        DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidOpenTextDocument,
        DidSaveTextDocument, Exit, Notification as _,
    },
    request::{
        CodeActionRequest, CodeLensRequest, CodeLensResolve, Completion as CompletionRequest,
        DocumentDiagnosticRequest, DocumentSymbolRequest, ExecuteCommand, FoldingRangeRequest,
        GotoDefinition, HoverRequest, References, Request as _, SemanticTokensFullRequest,
        Shutdown, WorkspaceConfiguration, WorkspaceDiagnosticRequest, WorkspaceSymbolRequest,
    },
};
use serde::de::DeserializeOwned;
use utils::{paths::AbsPathBuf, test_support::TestDir};

use crate::{
    Opt,
    config::{
        self,
        user_config::{DiagnosticsUpdateUserConfig, UserConfig},
    },
    global_state::main_loop,
    i18n::{I18n, Locale},
    lsp_ext::{
        ext::{
            EXPANDED_RENAME_COMMAND, ExpandedRenameParams, ProjectStatusNotification,
            RENAME_CONFLICT_INFO_COMMAND, RENAME_EXPANSION_INFO_COMMAND, RenameConflictInfoParams,
            RenameConflictInfoResult, RenameExpansionInfoParams, RenameExpansionInfoResult,
        },
        to_proto,
    },
};

type TempDir = TestDir;

const LSP_TEST_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_TEST_CONFIG: &str = "sources = [\"**\"]\ninclude_dirs = [\".\"]\n";
const SYNTAX_ONLY_TEST_CONFIG: &str = "\
# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill shell globs, for example sources = [\"rtl/**\"] and include_dirs = [\"include\"], to enable semantic diagnostics.
sources = []
include_dirs = []
";

fn recv_lsp_message_until(
    client: &Connection,
    deadline: Instant,
    context: &str,
) -> Option<Message> {
    let now = Instant::now();
    if now >= deadline {
        return None;
    }

    let timeout = deadline.saturating_duration_since(now);
    match client.receiver.recv_timeout(timeout) {
        Ok(message) => Some(message),
        Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
        Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
            panic!("test client disconnected while waiting for {context}");
        }
    }
}

fn handle_test_server_request(client: &Connection, request: Request, context: &str) {
    if request.method == lsp_types::request::WorkDoneProgressCreate::METHOD
        || request.method == lsp_types::request::WorkspaceDiagnosticRefresh::METHOD
    {
        client
            .sender
            .send(Message::Response(lsp_server::Response::new_ok(request.id, ())))
            .unwrap();
        return;
    }

    panic!("unexpected server request during {context}: {request:?}");
}

fn spawn_default_test_server(
    config: config::Config,
    server: Connection,
) -> thread::JoinHandle<anyhow::Result<()>> {
    thread::spawn(move || main_loop::main_loop(config, server, lsp_types::TraceValue::Off))
}

fn test_server_config(
    root_path: AbsPathBuf,
    client_caps: ClientCapabilities,
    user_config: UserConfig,
) -> config::Config {
    test_server_config_with_i18n(root_path, client_caps, user_config, I18n::default())
}

fn test_server_config_with_i18n(
    root_path: AbsPathBuf,
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    i18n: I18n,
) -> config::Config {
    let opt = Opt {
        process_name: "vide-test".to_string(),
        log: "error".to_string(),
        log_filename: None,
        profile_trace: None,
    };
    config::Config::new(
        opt,
        root_path.clone(),
        client_caps,
        vec![root_path],
        i18n,
        user_config,
        Vec::new(),
    )
}

fn spawn_test_workspace(
    root_path: AbsPathBuf,
    client_caps: ClientCapabilities,
    user_config: UserConfig,
) -> (Connection, thread::JoinHandle<anyhow::Result<()>>) {
    let config = test_server_config(root_path, client_caps, user_config);
    let (server, client) = Connection::memory();
    let server_thread = spawn_default_test_server(config, server);
    (client, server_thread)
}

fn open_test_document(client: &Connection, uri: Url, text: &str) {
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidOpenTextDocument::METHOD.to_string(),
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "systemverilog".to_string(),
                    version: 1,
                    text: text.to_owned(),
                },
            },
        )))
        .unwrap();
}

fn setup_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    setup_diagnostics_test_inner(client_caps, user_config, file_text, None)
}

fn setup_configured_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    setup_diagnostics_test_inner(client_caps, user_config, file_text, Some(DEFAULT_TEST_CONFIG))
}

fn setup_syntax_only_config_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    setup_diagnostics_test_inner(client_caps, user_config, file_text, Some(SYNTAX_ONLY_TEST_CONFIG))
}

fn setup_empty_config_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    setup_diagnostics_test_inner(client_caps, user_config, file_text, Some(""))
}

fn setup_diagnostics_test_inner(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    file_text: &str,
    config_text: Option<&str>,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Url) {
    let temp_dir = TempDir::new("diag-test");
    let file_path = temp_dir.path().join("broken.sv");
    fs::write(&file_path, file_text).unwrap();
    if let Some(config_text) = config_text {
        fs::write(temp_dir.path().join("vide.toml"), config_text).unwrap();
    }

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) = spawn_test_workspace(root_path, client_caps, user_config);

    let uri = to_proto::url_from_abs_path(file_path.as_path()).unwrap();
    open_test_document(&client, uri.clone(), file_text);

    (temp_dir, client, server_thread, uri)
}

fn setup_configured_multi_file_diagnostics_test(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    files: &[(&str, &str)],
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Vec<Url>) {
    setup_multi_file_diagnostics_test_inner(client_caps, user_config, files, true)
}

fn setup_multi_file_diagnostics_test_inner(
    client_caps: ClientCapabilities,
    user_config: UserConfig,
    files: &[(&str, &str)],
    write_config: bool,
) -> (TempDir, Connection, thread::JoinHandle<anyhow::Result<()>>, Vec<Url>) {
    let temp_dir = TempDir::new("diag-test");
    let mut uris = Vec::new();
    if write_config {
        fs::write(temp_dir.path().join("vide.toml"), DEFAULT_TEST_CONFIG).unwrap();
    }

    for (path, text) in files {
        let file_path = temp_dir.path().join(path);
        fs::write(&file_path, text).unwrap();
        uris.push(to_proto::url_from_abs_path(file_path.as_path()).unwrap());
    }

    let root_path = temp_dir.path().to_path_buf();
    let (client, server_thread) = spawn_test_workspace(root_path, client_caps, user_config);

    for ((_path, text), uri) in files.iter().zip(uris.iter()) {
        open_test_document(&client, uri.clone(), text);
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
        match client.receiver.recv_timeout(LSP_TEST_TIMEOUT).unwrap() {
            Message::Response(response) if response.id == shutdown_id => {
                assert!(response.error.is_none(), "{:?}", response.error);
                break;
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            Message::Notification(notification)
                if notification.method == ProjectStatusNotification::METHOD => {}
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
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    while let Some(message) = recv_lsp_message_until(client, deadline, "documentDiagnostic") {
        match message {
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
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            Message::Request(request) => {
                handle_test_server_request(client, request, "documentDiagnostic diagnostics test")
            }
            _ => {}
        }
    }

    panic!("documentDiagnostic response not received");
}

fn request_document_diagnostics(
    client: &Connection,
    uri: Url,
    request_id: i32,
) -> (Option<String>, Vec<lsp_types::Diagnostic>) {
    request_document_diagnostics_with_previous_result_id(client, uri, request_id, None)
}

fn request_document_diagnostics_with_previous_result_id(
    client: &Connection,
    uri: Url,
    request_id: i32,
    previous_result_id: Option<String>,
) -> (Option<String>, Vec<lsp_types::Diagnostic>) {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            DocumentDiagnosticRequest::METHOD.to_string(),
            DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
                identifier: None,
                previous_result_id,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();
    recv_document_diagnostics(client, request_id)
}

fn update_test_configuration(client: &Connection, settings: serde_json::Value) {
    client
        .sender
        .send(Message::Notification(Notification::new(
            DidChangeConfiguration::METHOD.to_string(),
            DidChangeConfigurationParams { settings: serde_json::Value::Null },
        )))
        .unwrap();

    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    while let Some(message) = recv_lsp_message_until(client, deadline, "configuration update") {
        match message {
            Message::Request(request) if request.method == WorkspaceConfiguration::METHOD => {
                client
                    .sender
                    .send(Message::Response(lsp_server::Response::new_ok(
                        request.id,
                        vec![settings],
                    )))
                    .unwrap();
                return;
            }
            Message::Request(request) => {
                handle_test_server_request(client, request, "configuration update")
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            _ => {}
        }
    }

    panic!("workspace configuration request not received");
}

fn request_goto_definition_uris(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    request_id: i32,
) -> Vec<Url> {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            GotoDefinition::METHOD.to_string(),
            GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, needle),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let definition: Option<GotoDefinitionResponse> =
        recv_response(client, request_id, "definition");
    definition.map(goto_definition_response_uris).unwrap_or_default()
}

fn request_reference_uris(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    request_id: i32,
) -> Vec<Url> {
    request_reference_uris_with_include_declaration(client, uri, text, needle, request_id, true)
}

fn request_reference_uris_with_include_declaration(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    request_id: i32,
    include_declaration: bool,
) -> Vec<Url> {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            References::METHOD.to_string(),
            lsp_types::ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, needle),
                },
                context: lsp_types::ReferenceContext { include_declaration },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let references: Option<Vec<lsp_types::Location>> =
        recv_response(client, request_id, "references");
    references.unwrap_or_default().into_iter().map(|location| location.uri).collect()
}

fn request_hover(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    request_id: i32,
) -> Option<Hover> {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            HoverRequest::METHOD.to_string(),
            HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, needle),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();

    recv_response(client, request_id, "hover")
}

fn request_completion_labels(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    request_id: i32,
) -> Vec<String> {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            CompletionRequest::METHOD.to_string(),
            CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, needle),
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
                context: None,
            },
        )))
        .unwrap();

    let completion: Option<CompletionResponse> = recv_response(client, request_id, "completion");
    match completion {
        Some(CompletionResponse::Array(items)) => {
            items.into_iter().map(|item| item.label).collect()
        }
        Some(CompletionResponse::List(list)) => {
            list.items.into_iter().map(|item| item.label).collect()
        }
        None => Vec::new(),
    }
}

fn request_rename_response(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    new_name: &str,
    request_id: i32,
) -> lsp_server::Response {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            lsp_types::request::Rename::METHOD.to_string(),
            lsp_types::RenameParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri },
                    position: position_of(text, needle),
                },
                new_name: new_name.to_owned(),
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();

    recv_raw_response(client, request_id, "rename")
}

fn request_rename(
    client: &Connection,
    uri: Url,
    text: &str,
    needle: &str,
    new_name: &str,
    request_id: i32,
) -> Option<lsp_types::WorkspaceEdit> {
    let response = request_rename_response(client, uri, text, needle, new_name, request_id);
    assert!(response.error.is_none(), "rename returned error: {:?}", response.error);
    serde_json::from_value(response.result.unwrap_or(serde_json::Value::Null))
        .unwrap_or_else(|err| panic!("failed to decode rename response: {err}"))
}

fn request_execute_command_response(
    client: &Connection,
    command: &str,
    arguments: Vec<serde_json::Value>,
    request_id: i32,
) -> lsp_server::Response {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            ExecuteCommand::METHOD.to_string(),
            lsp_types::ExecuteCommandParams {
                command: command.to_owned(),
                arguments,
                work_done_progress_params: WorkDoneProgressParams::default(),
            },
        )))
        .unwrap();

    recv_raw_response(client, request_id, "executeCommand")
}

fn request_workspace_diagnostic_report(
    client: &Connection,
    request_id: i32,
    previous_result_ids: Vec<lsp_types::PreviousResultId>,
) -> lsp_types::WorkspaceDiagnosticReport {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            WorkspaceDiagnosticRequest::METHOD.to_string(),
            WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids,
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    let result: WorkspaceDiagnosticReportResult =
        recv_response(client, request_id, "workspaceDiagnostic");
    let WorkspaceDiagnosticReportResult::Report(report) = result else {
        panic!("unexpected workspaceDiagnostic response: {result:?}");
    };
    report
}

fn request_workspace_diagnostic_uris(client: &Connection, request_id: i32) -> Vec<Url> {
    request_workspace_diagnostic_report(client, request_id, Vec::new())
        .items
        .into_iter()
        .map(|item| match item {
            lsp_types::WorkspaceDocumentDiagnosticReport::Full(full) => full.uri,
            lsp_types::WorkspaceDocumentDiagnosticReport::Unchanged(unchanged) => unchanged.uri,
        })
        .collect()
}

fn recv_publish_diagnostics_for_uri(client: &Connection, uri: &Url) -> Vec<lsp_types::Diagnostic> {
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    while let Some(message) = recv_lsp_message_until(client, deadline, "publishDiagnostics") {
        match message {
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
                handle_test_server_request(client, request, "publishDiagnostics diagnostics test")
            }
            _ => {}
        }
    }

    panic!("publishDiagnostics notification not received for {uri}");
}

fn recv_empty_publish_diagnostics_for_uri(
    client: &Connection,
    uri: &Url,
    context: &str,
) -> Vec<lsp_types::Diagnostic> {
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    let mut last_diagnostics = None;
    while let Some(message) = recv_lsp_message_until(client, deadline, context) {
        match message {
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD =>
            {
                let params =
                    serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                        .unwrap();
                if &params.uri == uri {
                    if params.diagnostics.is_empty() {
                        return params.diagnostics;
                    }
                    last_diagnostics = Some(params.diagnostics);
                }
            }
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Request(request) => handle_test_server_request(client, request, context),
            _ => {}
        }
    }

    panic!(
        "empty publishDiagnostics notification not received for {uri} during {context}; last diagnostics: {last_diagnostics:?}"
    );
}

fn recv_response<T: DeserializeOwned>(
    client: &Connection,
    request_id: lsp_server::RequestId,
    label: &str,
) -> T {
    let response = recv_raw_response(client, request_id, label);
    assert!(response.error.is_none(), "{label} returned error: {:?}", response.error);
    serde_json::from_value(response.result.unwrap_or(serde_json::Value::Null))
        .unwrap_or_else(|err| panic!("failed to decode {label} response: {err}"))
}

fn recv_raw_response(
    client: &Connection,
    request_id: lsp_server::RequestId,
    label: &str,
) -> lsp_server::Response {
    let deadline = Instant::now() + LSP_TEST_TIMEOUT;
    while let Some(message) = recv_lsp_message_until(client, deadline, label) {
        match message {
            Message::Response(response) if response.id == request_id => return response,
            Message::Notification(notification)
                if notification.method == lsp_types::notification::Progress::METHOD => {}
            Message::Notification(notification)
                if notification.method == lsp_types::notification::PublishDiagnostics::METHOD => {}
            Message::Request(request) => handle_test_server_request(client, request, label),
            _ => {}
        }
    }

    panic!("{label} response not received");
}

fn goto_definition_response_uris(response: GotoDefinitionResponse) -> Vec<Url> {
    match response {
        GotoDefinitionResponse::Scalar(location) => vec![location.uri],
        GotoDefinitionResponse::Array(locations) => {
            locations.into_iter().map(|location| location.uri).collect()
        }
        GotoDefinitionResponse::Link(links) => {
            links.into_iter().map(|location| location.target_uri).collect()
        }
    }
}

fn position_of(text: &str, needle: &str) -> Position {
    let offset = text.find(needle).unwrap_or_else(|| panic!("missing {needle:?}"));
    position_at_offset(text, offset)
}

fn position_at_offset(text: &str, offset: usize) -> Position {
    let line = text[..offset].bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    Position { line, character: (offset - line_start) as u32 }
}

fn range_of(text: &str, needle: &str) -> Range {
    let start = text.find(needle).unwrap_or_else(|| panic!("missing {needle:?}"));
    Range::new(position_at_offset(text, start), position_at_offset(text, start + needle.len()))
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
                            CodeActionKind::REFACTOR_EXTRACT,
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
    const CONTENT_MODIFIED_RETRIES: i32 = 5;

    for attempt in 0..=CONTENT_MODIFIED_RETRIES {
        let request_id = lsp_server::RequestId::from(request_id + attempt);
        client
            .sender
            .send(Message::Request(Request::new(
                request_id.clone(),
                CodeActionRequest::METHOD.to_string(),
                CodeActionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    range: Range::new(position, position),
                    context: context.clone(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: Default::default(),
                },
            )))
            .unwrap();

        let response = recv_raw_response(client, request_id, "codeAction");
        if response.error.is_none() {
            return serde_json::from_value(response.result.unwrap_or(serde_json::Value::Null))
                .unwrap_or_else(|err| panic!("failed to decode codeAction response: {err}"));
        }

        if is_content_modified(&response) && attempt < CONTENT_MODIFIED_RETRIES {
            continue;
        }

        panic!("codeAction returned error: {:?}", response.error);
    }

    unreachable!("codeAction retries should either return or panic")
}

fn request_code_actions_with_range(
    client: &Connection,
    uri: Url,
    range: Range,
    context: CodeActionContext,
    request_id: i32,
) -> Vec<CodeActionOrCommand> {
    const CONTENT_MODIFIED_RETRIES: i32 = 5;

    for attempt in 0..=CONTENT_MODIFIED_RETRIES {
        let request_id = lsp_server::RequestId::from(request_id + attempt);
        client
            .sender
            .send(Message::Request(Request::new(
                request_id.clone(),
                CodeActionRequest::METHOD.to_string(),
                CodeActionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    range,
                    context: context.clone(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: Default::default(),
                },
            )))
            .unwrap();

        let response = recv_raw_response(client, request_id, "codeAction");
        if response.error.is_none() {
            return serde_json::from_value(response.result.unwrap_or(serde_json::Value::Null))
                .unwrap_or_else(|err| panic!("failed to decode codeAction response: {err}"));
        }

        if is_content_modified(&response) && attempt < CONTENT_MODIFIED_RETRIES {
            continue;
        }

        panic!("codeAction returned error: {:?}", response.error);
    }

    unreachable!("codeAction retries should either return or panic")
}

fn is_content_modified(response: &lsp_server::Response) -> bool {
    response
        .error
        .as_ref()
        .is_some_and(|error| error.code == lsp_server::ErrorCode::ContentModified as i32)
}

fn request_code_lenses(client: &Connection, uri: Url, request_id: i32) -> Vec<lsp_types::CodeLens> {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            CodeLensRequest::METHOD.to_string(),
            lsp_types::CodeLensParams {
                text_document: TextDocumentIdentifier { uri },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        )))
        .unwrap();

    recv_response(client, request_id, "codeLens")
}

fn resolve_code_lens(
    client: &Connection,
    lens: lsp_types::CodeLens,
    request_id: i32,
) -> lsp_types::CodeLens {
    let request_id = lsp_server::RequestId::from(request_id);
    client
        .sender
        .send(Message::Request(Request::new(
            request_id.clone(),
            CodeLensResolve::METHOD.to_string(),
            lens,
        )))
        .unwrap();

    recv_response(client, request_id, "codeLens/resolve")
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

fn diagnostic_option(diagnostic: &lsp_types::Diagnostic) -> Option<&str> {
    diagnostic.data.as_ref()?.get("option")?.as_str()
}

mod analysis;
mod code_actions;
mod diagnostics;
mod localization;
mod navigation;
mod rename;
mod verilog_2005;
mod workspace;
mod workspace_symbols;
