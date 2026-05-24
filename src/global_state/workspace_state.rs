#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct WorkspaceGeneration(u64);

impl WorkspaceGeneration {
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceFetchCause {
    pub(crate) generation: WorkspaceGeneration,
    pub(crate) cause: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceFetchCompletion {
    CurrentSuccess,
    CurrentFailure,
    Stale { progress_started: bool },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct VfsProgress {
    pub(crate) config_version: u32,
    pub(crate) n_done: usize,
    pub(crate) n_total: usize,
}

impl VfsProgress {
    pub(crate) fn in_progress(&self) -> bool {
        self.n_done < self.n_total
    }
}

#[derive(Debug)]
pub(crate) struct WorkspaceVfsReadiness {
    requested_workspace_generation: WorkspaceGeneration,
    active_fetch_generation: Option<WorkspaceGeneration>,
    fetch_progress_generation: Option<WorkspaceGeneration>,
    committed_workspace_generation: WorkspaceGeneration,
    vfs_config_version: u32,
    vfs_progress: VfsProgress,
    vfs_ready: bool,
    diagnostic_readiness_revision: u64,
    diagnostics_deferred_until_ready: bool,
}

impl Default for WorkspaceVfsReadiness {
    fn default() -> Self {
        Self {
            requested_workspace_generation: WorkspaceGeneration::default(),
            active_fetch_generation: None,
            fetch_progress_generation: None,
            committed_workspace_generation: WorkspaceGeneration::default(),
            vfs_config_version: 0,
            vfs_progress: VfsProgress::default(),
            vfs_ready: true,
            diagnostic_readiness_revision: 0,
            diagnostics_deferred_until_ready: false,
        }
    }
}

impl WorkspaceVfsReadiness {
    pub(crate) fn request_workspace_reload(&mut self, cause: String) -> WorkspaceFetchCause {
        self.requested_workspace_generation = self.requested_workspace_generation.next();
        self.diagnostic_readiness_revision += 1;
        WorkspaceFetchCause { generation: self.requested_workspace_generation, cause }
    }

    pub(crate) fn start_workspace_fetch(&mut self, generation: WorkspaceGeneration) {
        self.active_fetch_generation = Some(generation);
    }

    pub(crate) fn accept_workspace_fetch_begin(&mut self, generation: WorkspaceGeneration) -> bool {
        let accepted = self.active_fetch_generation == Some(generation)
            && self.requested_workspace_generation == generation;
        if accepted {
            self.fetch_progress_generation = Some(generation);
        }
        accepted
    }

    pub(crate) fn finish_workspace_fetch(
        &mut self,
        generation: WorkspaceGeneration,
        has_errors: bool,
    ) -> WorkspaceFetchCompletion {
        if self.active_fetch_generation != Some(generation) {
            return WorkspaceFetchCompletion::Stale {
                progress_started: self.finish_fetch_progress(generation),
            };
        }

        self.active_fetch_generation = None;
        if self.requested_workspace_generation != generation {
            return WorkspaceFetchCompletion::Stale {
                progress_started: self.finish_fetch_progress(generation),
            };
        }

        self.finish_fetch_progress(generation);
        if has_errors {
            self.requested_workspace_generation = self.committed_workspace_generation;
            return WorkspaceFetchCompletion::CurrentFailure;
        }

        self.committed_workspace_generation = generation;
        WorkspaceFetchCompletion::CurrentSuccess
    }

    fn finish_fetch_progress(&mut self, generation: WorkspaceGeneration) -> bool {
        if self.fetch_progress_generation == Some(generation) {
            self.fetch_progress_generation = None;
            return true;
        }
        false
    }

    pub(crate) fn begin_vfs_load(&mut self, n_total: usize) -> u32 {
        self.vfs_config_version += 1;
        self.vfs_progress =
            VfsProgress { config_version: self.vfs_config_version, n_done: 0, n_total };
        self.vfs_ready = false;
        self.vfs_config_version
    }

    pub(crate) fn accept_vfs_progress(
        &mut self,
        config_version: u32,
        n_done: usize,
        n_total: usize,
    ) -> Option<VfsProgress> {
        if config_version != self.vfs_config_version {
            return None;
        }
        if n_done < self.vfs_progress.n_done {
            return None;
        }

        self.vfs_progress = VfsProgress { config_version, n_done, n_total };
        self.vfs_ready = !self.vfs_progress.in_progress();
        Some(self.vfs_progress)
    }

    pub(crate) fn accepts_vfs_loaded(&self, config_version: u32) -> bool {
        config_version == self.vfs_config_version
    }

    pub(crate) fn current_vfs_config_version(&self) -> u32 {
        self.vfs_config_version
    }

    #[cfg(test)]
    pub(crate) fn current_vfs_progress(&self) -> VfsProgress {
        self.vfs_progress
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.active_fetch_generation.is_none()
            && self.requested_workspace_generation == self.committed_workspace_generation
            && self.vfs_progress.config_version == self.vfs_config_version
            && self.vfs_ready
    }

    pub(crate) fn defer_diagnostics_until_ready(&mut self) {
        self.diagnostics_deferred_until_ready = true;
    }

    pub(crate) fn take_deferred_diagnostics_if_ready(&mut self) -> bool {
        if !self.is_ready() {
            return false;
        }

        std::mem::take(&mut self.diagnostics_deferred_until_ready)
    }

    pub(crate) fn diagnostic_readiness_revision(&self) -> u64 {
        self.diagnostic_readiness_revision
    }
}
