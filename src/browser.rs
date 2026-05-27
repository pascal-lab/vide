use crossbeam_channel::Receiver;
use itertools::Itertools;
use lsp_server::{Message, Request, Response};
use lsp_types::{
    InitializeParams, InitializeResult, MessageType, ServerInfo, ShowMessageParams, TraceValue,
    Url, notification::Notification as _, request::Request as _,
};
use utils::{
    json::from_json,
    paths::{AbsPathBuf, Utf8PathBuf},
};

use crate::{
    DEFAULT_PROCESS_NAME, Opt,
    config::Config,
    global_state::GlobalState,
    i18n::{I18n, Locale},
};

const BROWSER_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "_WASM");

#[derive(Default)]
pub struct BrowserServer {
    session: Option<BrowserSession>,
}

struct BrowserSession {
    state: GlobalState,
    outgoing: Receiver<Message>,
}

struct InitializeOutput {
    session: BrowserSession,
    messages: Vec<Message>,
}

impl BrowserServer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_message_json(&mut self, json: &str) -> Result<String, String> {
        let message: Message = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let mut emitted = Vec::new();

        if let Message::Request(request) = &message
            && request.method == lsp_types::request::Initialize::METHOD
        {
            if self.session.is_some() {
                return Err("Vide LSP session is already initialized".to_owned());
            }
            let mut initialized = initialize(request)?;
            emitted.append(&mut initialized.messages);
            self.session = Some(initialized.session);
            return serialize_messages(&emitted);
        }

        let Some(active) = self.session.as_mut() else {
            return Err("Vide LSP session must receive initialize first".to_owned());
        };

        active
            .state
            .handle_lsp_message_for_browser(message)
            .map_err(|error| format!("{error:#}"))?;
        emitted.extend(active.drain_outgoing());
        serialize_messages(&emitted)
    }

    pub fn poll_json(&mut self) -> Result<String, String> {
        let mut emitted = Vec::new();
        if let Some(active) = self.session.as_mut() {
            active.state.drain_browser_queued_events().map_err(|error| format!("{error:#}"))?;
            emitted.extend(active.drain_outgoing());
        }
        serialize_messages(&emitted)
    }

    pub fn reset(&mut self) {
        self.session = None;
    }
}

fn initialize(request: &Request) -> Result<InitializeOutput, String> {
    #[allow(deprecated)]
    let InitializeParams {
        root_uri,
        capabilities: client_caps,
        workspace_folders,
        initialization_options,
        trace,
        locale,
        ..
    } = from_json::<InitializeParams>("InitializeParams", &request.params)
        .map_err(|error| error.to_string())?;

    let root_path = root_uri.as_ref().and_then(abs_path_from_url).unwrap_or_else(default_root);
    let workspace_roots = workspace_folders
        .map(|folders| {
            folders.into_iter().filter_map(|folder| abs_path_from_url(&folder.uri)).collect_vec()
        })
        .filter(|folders| !folders.is_empty())
        .unwrap_or_else(|| vec![root_path.clone()]);

    let i18n = I18n::new(Locale::from_lsp(locale.as_deref()));
    let (user_config, snippets, config_errors) =
        initialization_options.map(Config::parse_initialization_options).unwrap_or_default();

    let config = Config::new(
        Opt {
            process_name: DEFAULT_PROCESS_NAME.to_owned(),
            log: "error".to_owned(),
            log_filename: None,
            profile_trace: None,
        },
        root_path,
        client_caps,
        workspace_roots,
        i18n,
        user_config,
        snippets,
    );

    let initialize_result = InitializeResult {
        capabilities: browser_server_caps(&config),
        server_info: Some(ServerInfo {
            name: DEFAULT_PROCESS_NAME.to_owned(),
            version: Some(BROWSER_VERSION.to_owned()),
        }),
    };

    let (sender, outgoing) = crossbeam_channel::unbounded();
    let mut state = GlobalState::new(sender, config, trace.unwrap_or(TraceValue::Off));
    state.request_workspace_reload("Start");
    state.start_requested_workspace_fetch();
    let mut messages = vec![Response::new_ok(request.id.clone(), &initialize_result).into()];

    if !config_errors.is_empty() {
        let notification = lsp_server::Notification::new(
            lsp_types::notification::ShowMessage::METHOD.to_owned(),
            ShowMessageParams { typ: MessageType::WARNING, message: config_errors.message(i18n) },
        );
        messages.push(notification.into());
    }

    Ok(InitializeOutput { session: BrowserSession { state, outgoing }, messages })
}

fn browser_server_caps(config: &Config) -> lsp_types::ServerCapabilities {
    let mut capabilities = config.server_caps();
    // Multiple browser labs can run in one page. vscode-languageclient registers
    // executeCommandProvider entries into the page-global VS Code command registry,
    // while these commands are desktop extension entry points.
    capabilities.execute_command_provider = None;
    capabilities
}

impl BrowserSession {
    fn drain_outgoing(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        while let Ok(message) = self.outgoing.try_recv() {
            messages.push(message);
        }
        messages
    }
}

fn serialize_messages(messages: &[Message]) -> Result<String, String> {
    serde_json::to_string(messages).map_err(|error| error.to_string())
}

fn default_root() -> AbsPathBuf {
    AbsPathBuf::assert(Utf8PathBuf::from("/workspace"))
}

fn abs_path_from_url(url: &Url) -> Option<AbsPathBuf> {
    if let Ok(path) = url.to_file_path() {
        return AbsPathBuf::try_from(path).ok();
    }

    let path = url.path();
    if path.is_empty() {
        return None;
    }
    AbsPathBuf::try_from(path).ok()
}

#[cfg(test)]
mod tests {
    use lsp_types::ClientCapabilities;
    use utils::test_support::TestDir;

    use super::{Config, I18n, Opt, browser_server_caps};
    use crate::{DEFAULT_PROCESS_NAME, config::user_config::UserConfig};

    #[test]
    fn browser_server_caps_do_not_advertise_execute_commands() {
        let root = TestDir::new("browser-caps");
        let root_path = root.path().to_path_buf();
        let config = Config::new(
            Opt {
                process_name: DEFAULT_PROCESS_NAME.to_owned(),
                log: "error".to_owned(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );

        assert!(config.server_caps().execute_command_provider.is_some());
        assert!(browser_server_caps(&config).execute_command_provider.is_none());
    }
}
