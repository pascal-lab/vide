use lsp_types::{
    DidChangeConfigurationParams, DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams,
};
use triomphe::Arc;

use crate::{
    DEFAULT_PROCESS_NAME,
    global_state::{GlobalState, reload},
    lsp_ext::from_proto,
};

pub(crate) fn handle_did_change_configuration(
    state: &mut GlobalState,
    // As stated in https://github.com/microsoft/language-server-protocol/issues/676,
    // this notification's parameters should be ignored and the actual config queried separately.
    _params: DidChangeConfigurationParams,
) -> anyhow::Result<()> {
    state.send_request::<lsp_types::request::WorkspaceConfiguration>(
        lsp_types::ConfigurationParams {
            items: vec![lsp_types::ConfigurationItem {
                scope_uri: None,
                section: Some(DEFAULT_PROCESS_NAME.into()),
            }],
        },
        |this, resp| {
            tracing::debug!("config update response: '{:?}", resp);
            let lsp_server::Response { result, error, .. } = resp;

            match (result, error) {
                (_, Some(err)) => {
                    tracing::error!("failed to fetch the server settings: {:?}", err)
                }
                (Some(mut configs), None) => {
                    if let Some(json) = configs.get_mut(0) {
                        // Note that json can be null according to the spec if the client can't
                        // provide a configuration. This is handled in Config::update below.
                        let mut config = (*this.config_state.config).clone();
                        this.config_state.config_errors = config.update(json.take()).err();
                        this.update_configuration(config);
                    }
                }
                (None, None) => {
                    tracing::error!("received empty server settings response from the client")
                }
            }
        },
    );

    Ok(())
}

pub(crate) fn handle_did_change_workspace_folders(
    state: &mut GlobalState,
    params: DidChangeWorkspaceFoldersParams,
) -> anyhow::Result<()> {
    let config = Arc::make_mut(&mut state.config_state.config);

    for workspace in params.event.removed {
        if let Ok(path) = from_proto::abs_path(&workspace.uri) {
            config.remove_workspace(&path);
        }
    }

    let added = params.event.added.into_iter().filter_map(|it| from_proto::abs_path(&it.uri).ok());
    config.add_workspaces(added);

    // TODO: ??
    config.refresh_project_manifests();
    state.request_workspace_reload("client workspaces changed");

    Ok(())
}

pub(crate) fn handle_did_change_watched_files(
    state: &mut GlobalState,
    params: DidChangeWatchedFilesParams,
) -> anyhow::Result<()> {
    let mut workspace_structure_change = None;

    for change in params.changes {
        if let Ok(path) = from_proto::abs_path(&change.uri) {
            if reload::should_refresh_for_change(
                &path,
                change.typ != lsp_types::FileChangeType::CHANGED,
            ) {
                workspace_structure_change.get_or_insert(path);
                continue;
            }

            // Invalidate the file in the VFS so that it's reloaded later.
            state.workspace.vfs_loader.handle.invalidate(path);
        }
    }

    if let Some(path) = workspace_structure_change {
        let config = Arc::make_mut(&mut state.config_state.config);
        config.refresh_project_manifests();
        state.request_workspace_auto_reload(format!("DidChangeWatchedFiles {path}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use lsp_server::Connection;
    use lsp_types::{DidChangeWatchedFilesParams, FileChangeType, FileEvent, TraceValue, Url};
    use project_model::project_manifest;
    use utils::{paths::AbsPathBuf, test_support::TestDir};

    use super::handle_did_change_watched_files;
    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    fn test_state_with_root(root_path: AbsPathBuf) -> (GlobalState, Connection) {
        let config = config::Config::new(
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
        (GlobalState::new(server.sender, config, TraceValue::Off), client)
    }

    #[test]
    fn watched_manifest_delete_requests_workspace_reload() {
        let root = TestDir::new("watched-manifest-delete");
        let (mut state, _client) = test_state_with_root(root.path().to_path_buf());
        let manifest_path = root.join(project_manifest::MANIFEST_FILE_NAME);
        let manifest_uri = Url::from_file_path(manifest_path.as_path()).unwrap();

        handle_did_change_watched_files(
            &mut state,
            DidChangeWatchedFilesParams {
                changes: vec![FileEvent::new(manifest_uri, FileChangeType::DELETED)],
            },
        )
        .unwrap();

        assert!(state.workspace.fetch_workspaces_task.has_op_requested());
    }
}
