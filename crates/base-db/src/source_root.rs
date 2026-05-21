use vfs::{FileId, FileSet, FileSetConfig, Vfs, VfsPath, anchored_path::AnchoredPath};

use crate::source_db::SourceFileKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceRootId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceRootRole {
    Local,
    BestEffortIndex,
    Library,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRootDiagnosticScope {
    Workspace,
    OpenFile,
    Disabled,
}

impl SourceRootRole {
    pub fn is_library(self) -> bool {
        matches!(self, SourceRootRole::Library)
    }

    pub fn is_ignored(self) -> bool {
        matches!(self, SourceRootRole::Ignored)
    }

    pub fn is_watched(self) -> bool {
        matches!(self, SourceRootRole::Local | SourceRootRole::BestEffortIndex)
    }

    pub fn participates_in_semantic_profile(self) -> bool {
        matches!(self, SourceRootRole::Local | SourceRootRole::Library)
    }

    pub fn supports_root_scoped_compilation(self) -> bool {
        self.participates_in_semantic_profile()
    }

    pub fn reports_missing_profile(self) -> bool {
        self.participates_in_semantic_profile()
    }

    pub fn diagnostic_scope(self) -> SourceRootDiagnosticScope {
        match self {
            SourceRootRole::Local | SourceRootRole::Library => SourceRootDiagnosticScope::Workspace,
            SourceRootRole::BestEffortIndex => SourceRootDiagnosticScope::OpenFile,
            SourceRootRole::Ignored => SourceRootDiagnosticScope::Disabled,
        }
    }

    pub fn allows_workspace_edits(self) -> bool {
        !matches!(self, SourceRootRole::BestEffortIndex)
    }

    pub fn publishes_unopened_workspace_diagnostics(self) -> bool {
        // Ignored roots still publish empty reports so clients can clear stale
        // diagnostics from an earlier configuration.
        !matches!(self, SourceRootRole::BestEffortIndex)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRoot {
    role: SourceRootRole,
    source_files: Option<Vec<FileId>>,
    file_set: FileSet,
}

impl SourceRoot {
    pub fn new(role: SourceRootRole, file_set: FileSet) -> SourceRoot {
        SourceRoot { role, source_files: None, file_set }
    }

    pub fn with_source_files(
        role: SourceRootRole,
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot { role, source_files: Some(source_files), file_set }
    }

    pub fn new_local(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Local, file_set)
    }

    pub fn new_library(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Library, file_set)
    }

    pub fn new_best_effort_index(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::BestEffortIndex, file_set)
    }

    pub fn new_ignored(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Ignored, file_set)
    }

    pub fn new_local_with_source_files(file_set: FileSet, source_files: Vec<FileId>) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::Local, file_set, source_files)
    }

    pub fn new_library_with_source_files(
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::Library, file_set, source_files)
    }

    pub fn new_best_effort_index_with_source_files(
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::BestEffortIndex, file_set, source_files)
    }

    pub fn role(&self) -> SourceRootRole {
        self.role
    }

    pub fn is_library(&self) -> bool {
        self.role.is_library()
    }

    pub fn is_ignored(&self) -> bool {
        self.role.is_ignored()
    }

    pub fn path_for_file(&self, file: &FileId) -> Option<&VfsPath> {
        self.file_set.get_path(file)
    }

    pub fn file_for_path(&self, path: &VfsPath) -> Option<&FileId> {
        self.file_set.get_file(path)
    }

    pub fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        self.file_set.resolve_path(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = FileId> + '_ {
        self.file_set.iter()
    }

    pub fn file_kind(&self, file: &FileId) -> SourceFileKind {
        let Some(path) = self.path_for_file(file) else {
            return SourceFileKind::default();
        };
        let kind = SourceFileKind::from_path(path);
        let Some(source_files) = &self.source_files else {
            return kind;
        };
        if matches!(kind, SourceFileKind::LibraryMap | SourceFileKind::ProjectManifest)
            || source_files.contains(file)
        {
            kind
        } else {
            SourceFileKind::IncludeHeader
        }
    }
}

#[derive(Debug)]
pub struct SourceRootConfig {
    pub fileset_config: FileSetConfig,
    pub fileset_roles: Vec<SourceRootRole>,
}

impl Default for SourceRootConfig {
    fn default() -> Self {
        Self {
            fileset_config: FileSetConfig::default(),
            fileset_roles: vec![SourceRootRole::Ignored],
        }
    }
}

impl SourceRootConfig {
    pub fn partition(&self, vfs: &Vfs) -> Vec<SourceRoot> {
        let partitions = self.fileset_config.partition_with_source(vfs);
        debug_assert_eq!(
            self.fileset_roles.len(),
            partitions.len(),
            "source root roles must track file-set partitions",
        );

        let partition_len = partitions.len();
        partitions
            .into_iter()
            .enumerate()
            .map(|(idx, partition)| {
                let file_set = partition.file_set;
                let fallback_role = if idx + 1 == partition_len {
                    SourceRootRole::Ignored
                } else {
                    SourceRootRole::Library
                };
                let role = self.fileset_roles.get(idx).copied().unwrap_or(fallback_role);
                let source_files =
                    partition.source_files.map(|source_files| source_files.into_iter().collect());
                if role.is_ignored() {
                    return SourceRoot::new(role, file_set);
                }

                match source_files {
                    Some(source_files) => {
                        SourceRoot::with_source_files(role, file_set, source_files)
                    }
                    None => SourceRoot::new(role, file_set),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_local_fileset_without_source_paths_remains_library() {
        let mut builder = FileSetConfig::builder();
        builder.add_file_set(vec![VfsPath::new_virtual_path("/lib".into())]);
        let config = SourceRootConfig {
            fileset_config: builder.build(),
            fileset_roles: vec![SourceRootRole::Library, SourceRootRole::Ignored],
        };
        let roots = config.partition(&Vfs::default());

        assert_eq!(roots[0].role(), SourceRootRole::Library);
        assert_eq!(roots[1].role(), SourceRootRole::Ignored);
    }

    #[test]
    fn ignored_root_preserves_file_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/ignored/file.sv".into()));
        let root = SourceRoot::new_ignored(file_set);

        assert_eq!(root.role(), SourceRootRole::Ignored);
        assert_eq!(root.file_kind(&file_id), SourceFileKind::SystemVerilog);
    }

    #[test]
    fn best_effort_index_root_preserves_file_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/indexed/file.sv".into()));
        let root = SourceRoot::new_best_effort_index(file_set);

        assert_eq!(root.role(), SourceRootRole::BestEffortIndex);
        assert_eq!(root.file_kind(&file_id), SourceFileKind::SystemVerilog);
    }

    #[test]
    fn source_root_role_policies_are_explicit() {
        assert!(SourceRootRole::Local.participates_in_semantic_profile());
        assert!(SourceRootRole::Library.participates_in_semantic_profile());
        assert!(!SourceRootRole::BestEffortIndex.participates_in_semantic_profile());
        assert!(!SourceRootRole::Ignored.participates_in_semantic_profile());

        assert!(SourceRootRole::Local.supports_root_scoped_compilation());
        assert!(!SourceRootRole::BestEffortIndex.supports_root_scoped_compilation());

        assert_eq!(SourceRootRole::Local.diagnostic_scope(), SourceRootDiagnosticScope::Workspace);
        assert_eq!(
            SourceRootRole::BestEffortIndex.diagnostic_scope(),
            SourceRootDiagnosticScope::OpenFile,
        );
        assert_eq!(SourceRootRole::Ignored.diagnostic_scope(), SourceRootDiagnosticScope::Disabled);

        assert!(SourceRootRole::Local.allows_workspace_edits());
        assert!(!SourceRootRole::BestEffortIndex.allows_workspace_edits());
        assert!(!SourceRootRole::BestEffortIndex.publishes_unopened_workspace_diagnostics());
    }

    #[test]
    fn source_filtered_root_preserves_project_manifest_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/root/vizsla.toml".into()));
        let root = SourceRoot::new_local_with_source_files(file_set, Vec::new());

        assert_eq!(root.file_kind(&file_id), SourceFileKind::ProjectManifest);
    }
}
