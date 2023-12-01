use vfs::{
    file_set::{FileSet, FileSetConfig},
    vfs::{AnchoredPath, FileId, Vfs, VfsPath},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceRootId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRoot {
    pub is_lib: bool,
    file_set: FileSet,
}

impl SourceRoot {
    pub fn new_local(file_set: FileSet) -> SourceRoot {
        SourceRoot { is_lib: false, file_set }
    }

    pub fn new_library(file_set: FileSet) -> SourceRoot {
        SourceRoot { is_lib: true, file_set }
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
}

#[derive(Default, Debug)]
pub struct SourceRootConfig {
    pub fileset_config: FileSetConfig,
    pub local_filesets: Vec<usize>,
}

impl SourceRootConfig {
    pub fn partition(&self, vfs: &Vfs) -> Vec<SourceRoot> {
        self.fileset_config
            .partition(vfs)
            .into_iter()
            .enumerate()
            .map(|(idx, file_set)| {
                if self.local_filesets.contains(&idx) {
                    SourceRoot::new_local(file_set)
                } else {
                    SourceRoot::new_library(file_set)
                }
            })
            .collect()
    }
}
