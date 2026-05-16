use base_db::{change::Change, source_db::SourceDb};
use itertools::Itertools;
use project_model::{
    ProjectModel, Workspace, get_workspace_folder, project_manifest::MANIFEST_FILE_NAME,
};
use triomphe::Arc;
use utils::{paths::AbsPath, thread::ThreadIntent};

use super::main_loop::Task;
use crate::{
    config::{Config, FilesWatcher},
    global_state::{DEFAULT_REQ_HANDLER, GlobalState, process_changes::DiagnosticInvalidation},
};

#[derive(Debug)]
pub(crate) enum FetchWorkspaceProgress {
    Begin,
    // workspaces
    End(Vec<Workspace>, Vec<anyhow::Error>),
}

impl From<FetchWorkspaceProgress> for Task {
    fn from(value: FetchWorkspaceProgress) -> Self {
        Task::FetchWorkspace(value)
    }
}

impl GlobalState {
    pub(crate) fn is_stuck(&self) -> bool {
        !(self.fetch_workspaces_task.in_process()
            || self.vfs_progress.config_version < self.vfs_config_version
            || self.vfs_progress.in_progress())
    }

    pub(crate) fn fetch_workspaces(&mut self, cause: String) {
        tracing::info!(%cause, "will fetch workspaces");

        self.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, {
            let manifests = self.config.project_manifests.clone();

            move |sender| {
                if sender.send(FetchWorkspaceProgress::Begin.into()).is_err() {
                    tracing::debug!("workspace fetch start dropped because main loop is gone");
                    return;
                }

                let (project_model, error_sink) = ProjectModel::load(manifests);
                let all_workspaces = project_model.workspaces;

                tracing::info!("did fetch workspaces {:?}", all_workspaces);

                if !error_sink.is_empty() {
                    tracing::error!("failed to fetch workspaces {:?}", error_sink);
                }

                if sender
                    .send(FetchWorkspaceProgress::End(all_workspaces, error_sink).into())
                    .is_err()
                {
                    tracing::debug!("workspace fetch result dropped because main loop is gone");
                }
            }
        })
    }

    pub(crate) fn fetch_workspace_error_stringify(&self) -> Result<(), String> {
        match self.fetch_workspaces_task.last_op_result() {
            Some((workspaces, _)) if workspaces.is_empty() => Err("no workspace fetched".into()),
            Some((_, errors)) if !errors.is_empty() => Err(errors
                .iter()
                .map(|err| format!("failed to load workspace {:#}", err))
                .join("\n")),
            _ => Ok(()),
        }
    }

    pub(crate) fn switch_workspaces(&mut self, cause: String) {
        tracing::info!(%cause, "start switching workspaces");

        let Some((workspaces, errors)) = self.fetch_workspaces_task.last_op_result() else {
            return;
        };

        if !errors.is_empty() && !self.workspaces.is_empty() {
            return;
        }

        self.workspaces = workspaces.clone();

        if let FilesWatcher::Client = self.config.files().watcher {
            let registration_options = lsp_types::DidChangeWatchedFilesRegistrationOptions {
                watchers: self
                    .workspaces
                    .iter()
                    .flat_map(|ws| ws.to_roots())
                    .filter(|it| !it.is_lib)
                    .flat_map(|root| {
                        root.load_paths().into_iter().flat_map(|it| {
                            [
                                format!("{it}/**/*.v"),
                                format!("{it}/**/*.sv"),
                                format!("{it}/**/*.vh"),
                                format!("{it}/**/*.svh"),
                                format!("{it}/**/*.svi"),
                                format!("{it}/**/{}", MANIFEST_FILE_NAME),
                            ]
                        })
                    })
                    .map(|glob_pattern| lsp_types::FileSystemWatcher {
                        glob_pattern: lsp_types::GlobPattern::String(glob_pattern),
                        kind: None,
                    })
                    .collect(),
            };

            match serde_json::to_value(registration_options) {
                Ok(register_options) => {
                    let registration = lsp_types::Registration {
                        id: "workspace/didChangeWatchedFiles".to_string(),
                        method: "workspace/didChangeWatchedFiles".to_string(),
                        register_options: Some(register_options),
                    };

                    self.send_request::<lsp_types::request::RegisterCapability>(
                        lsp_types::RegistrationParams { registrations: vec![registration] },
                        DEFAULT_REQ_HANDLER,
                    );
                }
                Err(error) => {
                    tracing::error!(
                        "failed to serialize file watcher registration options: {error:#}"
                    );
                }
            }
        }

        let files_config = self.config.files();
        let (to_load, to_watch, source_root_config, project_config) =
            get_workspace_folder(&self.workspaces, &files_config.exclude);
        let mut change = Change::new();
        {
            let vfs = self.vfs.read();
            change.set_roots(source_root_config.partition(&vfs.0));
        }
        self.project_config = project_config.clone();
        change.set_project_config(project_config);
        self.analysis_host.apply_change(change);

        let to_watch = match files_config.watcher {
            FilesWatcher::Client => vec![],
            FilesWatcher::Server => to_watch,
        };

        self.vfs_config_version += 1;

        self.vfs_loader.handle.set_config(vfs::loader::Config {
            to_load,
            to_watch,
            version: self.vfs_config_version,
        });

        self.source_root_config = source_root_config;

        self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);

        tracing::info!("did switch workspaces");
    }

    pub(crate) fn update_configuration(&mut self, config: Config) {
        let diagnostics_config = Arc::new(config.diagnostics_config());
        let _old_config = std::mem::replace(&mut self.config, Arc::new(config));
        self.analysis_host.raw_db_mut().set_diagnostics_config_with_durability(
            diagnostics_config,
            base_db::salsa::Durability::HIGH,
        );
        self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);
        // TODO: update LRU capacity
    }
}

pub(crate) fn should_refresh_for_change(path: &AbsPath, has_structure_change: bool) -> bool {
    let Some(file_name) = path.file_name() else {
        return false;
    };

    if file_name == MANIFEST_FILE_NAME {
        return true;
    }

    if !has_structure_change {
        return false;
    }

    false
}
