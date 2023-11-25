use std::{collections::VecDeque, iter};

use itertools::Itertools;
use project_model::{project_manifest::ProjectManifest, workspace::Workspace};
use rustc_hash::FxHashSet;
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
            let mut manifests = self.config.discovered_manifests.clone();
            let detached_files = self.config.detached_files.clone();

            move |sender| {
                sender.send(FetchWorkspaceProgress::Begin.into()).unwrap();

                let mut loaded_manifests = FxHashSet::default();
                let mut all_workspaces = Vec::new();
                let mut error_sink = Vec::new();
                let mut is_lib = false;

                while !manifests.is_empty() {
                    // Load workspaces
                    let (workspaces, errors): (Vec<_>, Vec<_>) = manifests
                        .iter()
                        .map(|mani| Workspace::load(mani, is_lib))
                        .partition_result();

                    error_sink.extend(errors);
                    loaded_manifests.extend(manifests);

                    // Get libraries from loaded workspaces
                    let (lib_manifests, errors): (Vec<_>, Vec<_>) = workspaces
                        .iter()
                        .filter_map(|it| match it {
                            Workspace::Project(it) => Some(&it.package_files),
                            Workspace::DetachedFiles(_) => None,
                        })
                        .flatten()
                        .map(ProjectManifest::discover)
                        .partition_result();

                    all_workspaces.extend(workspaces);
                    error_sink.extend(errors);

                    manifests = lib_manifests
                        .into_iter()
                        .flatten()
                        .filter(|mani| loaded_manifests.contains(mani))
                        .collect_vec();

                    is_lib = true;
                }

                if !detached_files.is_empty() {
                    match Workspace::load_detached_files(detached_files) {
                        Ok(ws) => all_workspaces.push(ws),
                        Err(err) => error_sink.push(err),
                    }
                }

                tracing::info!("did fetch workspaces {:?}", all_workspaces);

                if !error_sink.is_empty() {
                    tracing::error!("failed to fetch workspaces {:?}", error_sink);
                }

                sender
                    .send(FetchWorkspaceProgress::End(all_workspaces, error_sink).into())
                    .unwrap();
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

        self.workspaces = workspaces.clone();

        todo!("switch workspaces");
        tracing::info!("did switch workspaces");
    }
}
