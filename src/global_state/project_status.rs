use lsp_types::Url;
use project_model::project_manifest::ProjectManifest;
use utils::paths::AbsPath;

use super::GlobalState;
use crate::lsp_ext::ext::{ProjectStatusNotification, ProjectStatusParams, ProjectStatusState};

impl GlobalState {
    pub(crate) fn send_loading_project_status(&self, cause: String) {
        self.send_project_status(
            ProjectStatusState::Loading,
            self.workspaces.len(),
            Vec::new(),
            Some(cause),
        );
    }

    pub(crate) fn send_project_status_for_result(&self, workspace_count: usize, errors: &[String]) {
        let state = if !errors.is_empty() {
            ProjectStatusState::Error
        } else if self
            .config
            .project_manifests
            .iter()
            .any(|manifest| matches!(manifest, ProjectManifest::Toml(_)))
        {
            ProjectStatusState::Loaded
        } else {
            ProjectStatusState::NoManifest
        };

        self.send_project_status(state, workspace_count, errors.to_vec(), None);
    }

    fn send_project_status(
        &self,
        state: ProjectStatusState,
        workspace_count: usize,
        errors: Vec<String>,
        message: Option<String>,
    ) {
        let mut manifest_uris = Vec::new();
        let mut unconfigured_root_uris = Vec::new();

        for manifest in &self.config.project_manifests {
            match manifest {
                ProjectManifest::Toml(path) => {
                    if let Some(uri) = url_from_path(path.as_path()) {
                        manifest_uris.push(uri);
                    }
                }
                ProjectManifest::UnconfiguredRoot(path) => {
                    if let Some(uri) = url_from_path(path.as_path()) {
                        unconfigured_root_uris.push(uri);
                    }
                }
            }
        }

        self.send_notification::<ProjectStatusNotification>(ProjectStatusParams {
            state,
            manifest_uris,
            unconfigured_root_uris,
            workspace_count,
            errors,
            message,
        });
    }
}

fn url_from_path(path: &AbsPath) -> Option<Url> {
    Url::from_file_path(path).ok()
}

#[cfg(test)]
mod tests {
    use lsp_server::{Connection, Message, Notification as LspNotification};
    use lsp_types::notification::Notification as _;
    use project_model::project_manifest::MANIFEST_FILE_NAME;
    use utils::{paths::AbsPathBuf, test_support::TestDir};

    use crate::{
        Opt,
        config::{Config, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
        lsp_ext::ext::{ProjectStatusNotification, ProjectStatusParams, ProjectStatusState},
    };

    fn test_state_with_root(root_path: AbsPathBuf) -> (GlobalState, Connection) {
        let config = Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );

        let (server, client) = Connection::memory();
        (GlobalState::new(server.sender, config, lsp_types::TraceValue::Off), client)
    }

    fn drain_client_notifications(client: &Connection) -> Vec<LspNotification> {
        client
            .receiver
            .try_iter()
            .filter_map(|message| match message {
                Message::Notification(notification) => Some(notification),
                _ => None,
            })
            .collect()
    }

    fn project_status_notification(client: &Connection) -> ProjectStatusParams {
        let notification = drain_client_notifications(client)
            .into_iter()
            .find(|notification| notification.method == ProjectStatusNotification::METHOD)
            .expect("missing project status notification");
        serde_json::from_value(notification.params).unwrap()
    }

    #[test]
    fn project_status_reports_missing_manifest() {
        let dir = TestDir::new("project-status-missing");
        let (state, client) = test_state_with_root(dir.path().to_path_buf());

        state.send_project_status_for_result(1, &[]);

        let status = project_status_notification(&client);
        assert!(matches!(status.state, ProjectStatusState::NoManifest));
        assert!(status.manifest_uris.is_empty());
        assert_eq!(status.unconfigured_root_uris.len(), 1);
    }

    #[test]
    fn project_status_reports_loaded_manifest() {
        let dir = TestDir::new("project-status-loaded");
        let manifest_path = dir.join(MANIFEST_FILE_NAME);
        std::fs::write(&manifest_path, "").unwrap();
        let (state, client) = test_state_with_root(dir.path().to_path_buf());

        state.send_project_status_for_result(1, &[]);

        let status = project_status_notification(&client);
        assert!(matches!(status.state, ProjectStatusState::Loaded));
        assert_eq!(status.manifest_uris.len(), 1);
        assert!(status.unconfigured_root_uris.is_empty());
    }
}
