#![feature(try_blocks)]
use std::{env, fs, io, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use config::Config;
use const_format::formatcp;
use itertools::Itertools;
use lsp_server::Connection;
use lsp_types::{MessageType, ShowMessageParams};
use slang as _;
use tracing_subscriber::{
    Registry, filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};
use utils::{
    json::from_json,
    paths::{AbsPathBuf, patch_path_prefix},
};

use crate::global_state::main_loop;

mod config;
mod global_state;
mod lsp_ext;

const DEFAULT_PROCESS_NAME: &str = env!("CARGO_PKG_NAME");
const DEBUG: bool = cfg!(debug_assertions);
const VERSION: &str =
    formatcp!("{}_{}", env!("CARGO_PKG_VERSION"), if DEBUG { "DEBUG" } else { "RELEASE" });

#[derive(Clone, Debug, Parser)]
#[clap(name = DEFAULT_PROCESS_NAME, version = VERSION)]
pub struct Opt {
    #[clap(long, default_value = DEFAULT_PROCESS_NAME)]
    pub process_name: String,

    #[clap(short, long, default_value = formatcp!("{}", if DEBUG { "debug" } else { "error" }))]
    pub log: String,

    #[clap(long = "log_file", default_value = None)]
    pub log_filename: Option<PathBuf>,
}

fn setup_logging(opt: &Opt) -> anyhow::Result<()> {
    let target: Targets =
        opt.log.parse().with_context(|| format!("invalid log filter: `{}`", opt.log))?;

    let writer = match &opt.log_filename {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("could not create log directory: {}", parent.display())
                })?;
            }
            let file = fs::File::create(path)
                .with_context(|| format!("could not create log file: {}", path.display()))?;
            BoxMakeWriter::new(std::sync::Arc::new(file))
        }
        None => BoxMakeWriter::new(io::stderr),
    };

    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(writer);

    Registry::default().with(target).with(fmt_layer).init();

    Ok(())
}

#[allow(deprecated)]
fn run_server(opt: Opt) -> anyhow::Result<()> {
    tracing::info!("Server {}_{} started.", &opt.process_name, VERSION);

    // Start connection
    let (connection, io_threads) = Connection::stdio();

    // Initialize server
    let (initialize_id, initialize_params) = match connection.initialize_start() {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };

    tracing::info!("Server initialized. InitializeParams: {}", &initialize_params);

    let lsp_types::InitializeParams {
        root_uri,
        capabilities: client_caps,
        workspace_folders,
        initialization_options,
        ..
    } = from_json::<lsp_types::InitializeParams>("InitializeParams", &initialize_params)?;

    let root_path = match root_uri
        .and_then(|uri| uri.to_file_path().ok())
        .map(patch_path_prefix)
        .and_then(|path| AbsPathBuf::try_from(path).ok())
    {
        Some(path) => path,
        None => AbsPathBuf::assert_utf8(env::current_dir()?),
    };

    let workspace_roots = workspace_folders
        .map(|workspace| {
            workspace
                .into_iter()
                .filter_map(|folder| folder.uri.to_file_path().ok())
                .map(patch_path_prefix)
                .filter_map(|path| AbsPathBuf::try_from(path).ok())
                .collect_vec()
        })
        .filter(|folders| !folders.is_empty())
        .unwrap_or_else(|| vec![root_path.clone()]);

    let (user_config, snippets) = if let Some(options) = initialization_options {
        let (user_config, snippets, errors) = Config::parse_initialization_options(options);
        if !errors.is_empty() {
            use lsp_types::notification::{Notification, ShowMessage};
            let noti = lsp_server::Notification::new(
                ShowMessage::METHOD.to_string(),
                ShowMessageParams { typ: MessageType::WARNING, message: errors.to_string() },
            );
            connection.sender.send(lsp_server::Message::Notification(noti)).unwrap();
        }
        (user_config, snippets)
    } else {
        Default::default()
    };

    let config = Config::new(opt, root_path, client_caps, workspace_roots, user_config, snippets);

    let initialize_result = lsp_types::InitializeResult {
        capabilities: config.server_caps(),
        server_info: Some(lsp_types::ServerInfo {
            name: config.opt.process_name.clone(),
            version: Some(VERSION.to_string()),
        }),
    };

    let initialize_result = serde_json::to_value(initialize_result)?;

    if let Err(e) = connection.initialize_finish(initialize_id, initialize_result) {
        if e.channel_is_disconnected() {
            io_threads.join()?;
        }
        return Err(e.into());
    }

    main_loop::main_loop(config, connection)?;

    io_threads.join()?;
    tracing::info!("Server shut down. BYE!");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        unsafe {
            env::set_var("RUST_BACKTRACE", "short");
        }
    }

    let opt = Opt::parse();
    setup_logging(&opt)?;
    run_server(opt)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use lsp_server::{Connection, Message, Notification, Request};
    use lsp_types::{
        ClientCapabilities, DiagnosticClientCapabilities, DidOpenTextDocumentParams,
        DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
        ProgressParams, PublishDiagnosticsParams, TextDocumentClientCapabilities,
        TextDocumentIdentifier, TextDocumentItem, Url, WorkDoneProgressParams,
        notification::{DidOpenTextDocument, Exit, Notification as _},
        request::{DocumentDiagnosticRequest, Request as _, Shutdown},
    };
    use utils::paths::AbsPathBuf;

    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::main_loop,
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

        let uri = Url::from_file_path(&file_path).unwrap();
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

    fn shutdown_test_server(
        client: &Connection,
        server_thread: thread::JoinHandle<anyhow::Result<()>>,
    ) {
        let shutdown_id = lsp_server::RequestId::from(2);
        client
            .sender
            .send(Message::Request(Request::new(
                shutdown_id.clone(),
                Shutdown::METHOD.to_string(),
                (),
            )))
            .unwrap();

        loop {
            match client.receiver.recv_timeout(Duration::from_secs(10)).unwrap() {
                Message::Response(response) if response.id == shutdown_id => break,
                Message::Notification(notification)
                    if notification.method == lsp_types::notification::Progress::METHOD => {}
                Message::Notification(notification)
                    if notification.method
                        == lsp_types::notification::PublishDiagnostics::METHOD => {}
                other => panic!("unexpected message while shutting down test server: {other:?}"),
            }
        }

        client
            .sender
            .send(Message::Notification(Notification::new(Exit::METHOD.to_string(), ())))
            .unwrap();

        server_thread.join().unwrap().unwrap();
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
        let (_temp_dir, client, server_thread, uri) = setup_diagnostics_test(
            pull_caps,
            UserConfig::default(),
            "module broken(;\nendmodule\n",
        );
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
                    if notification.method
                        == lsp_types::notification::PublishDiagnostics::METHOD =>
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
        assert!(
            !saw_publish_diagnostics,
            "pull-capable client should not receive publishDiagnostics"
        );

        let quiet_until = std::time::Instant::now() + Duration::from_millis(500);
        while std::time::Instant::now() < quiet_until {
            let timeout = quiet_until.saturating_duration_since(std::time::Instant::now());
            match client.receiver.recv_timeout(timeout) {
                Ok(Message::Notification(notification))
                    if notification.method
                        == lsp_types::notification::PublishDiagnostics::METHOD =>
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
                    if notification.method
                        == lsp_types::notification::PublishDiagnostics::METHOD =>
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
        let user_config =
            UserConfig { semantic_diagnostics_enable: false, ..UserConfig::default() };
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
}
