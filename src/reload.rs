use std::iter;

use itertools::Itertools;
use project_model::workspace::Workspace;
use triomphe::Arc;
use utils::thread::ThreadIntent;

use crate::{global_state::GlobalState, main_loop::Task};

#[derive(Debug)]
pub(crate) enum FetchWorkspaceProgress {
    Begin,
    // workspaces, force_package_graph_reload
    End(Vec<Workspace>, Vec<anyhow::Error>),
}

impl From<FetchWorkspaceProgress> for Task {
    fn from(value: FetchWorkspaceProgress) -> Self {
        Task::FetchWorkspace(value)
    }
}

impl GlobalState {
    pub(crate) fn fetch_workspaces(&mut self, cause: String) {
        tracing::info!(%cause, "will fetch workspaces");

        self.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, {
            let projects = self.config.discovered_workspaces.clone();
            let detached_files = self.config.detached_files.clone();

            move |sender| {
                sender.send(FetchWorkspaceProgress::Begin.into()).unwrap();

                let workspaces = projects.iter().map(Workspace::load);

                let (workspaces, errors): (Vec<_>, Vec<_>) = if detached_files.is_empty() {
                    workspaces.partition_result()
                } else {
                    let detached_workspace = Workspace::load_detached_files(&detached_files);
                    workspaces.chain(iter::once(detached_workspace)).partition_result()
                };

                tracing::info!("did fetch workspaces {:?}", workspaces);

                if !errors.is_empty() {
                    tracing::error!("failed to fetch workspaces {:?}", errors);
                }

                sender.send(FetchWorkspaceProgress::End(workspaces, errors).into()).unwrap();
            }
        })
    }

    pub(crate) fn fetch_workspace_error_stringify(&self) -> Result<(), String> {
        match self.fetch_workspace_task.last_op_result() {
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

        let Some((workspaces, errors)) = self.fetch_workspace_task.last_op_result() else {
            return;
        };

        if !errors.is_empty() && !self.workspaces.is_empty() {
            return;
        }

        self.workspaces = Arc::new(workspaces.clone());

        // file_watcher
        todo!();
        tracing::info!("did switch workspaces");
    }
}
