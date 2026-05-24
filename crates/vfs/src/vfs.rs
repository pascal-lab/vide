use std::{fmt, mem};

use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::{
    lines::LineEnding,
    path_identity::{FileIdentityKey, PathKey, path_alias_paths},
};

use crate::{loader::LoadResult, vfs_path::VfsPath};

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileId(pub u32);

impl nohash_hasher::IsEnabled for FileId {}

#[derive(Default)]
pub struct Vfs {
    identities: VfsIdentityIndex,
    file_states: Vec<FileState>,
    changes: Vec<ChangedFile>,
}

#[derive(PartialEq, PartialOrd, Clone)]
pub enum FileState {
    Exists(String, LineEnding),
    Deleted,
}

#[derive(Debug)]
pub struct ChangedFile {
    pub file_id: FileId,
    pub change_kind: ChangeKind,
}

impl ChangedFile {
    pub fn is_created_or_deleted(&self) -> bool {
        matches!(self.change_kind, ChangeKind::Create(_, _) | ChangeKind::Delete)
    }

    pub fn text(&self) -> Option<Arc<str>> {
        match &self.change_kind {
            ChangeKind::Create(text, _) | ChangeKind::Modify(text, _) => Some(text.clone()),
            ChangeKind::Delete => None,
        }
    }

    pub fn ending(&self) -> Option<LineEnding> {
        match &self.change_kind {
            ChangeKind::Create(_, ending) | ChangeKind::Modify(_, ending) => Some(*ending),
            ChangeKind::Delete => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChangeKind {
    Create(Arc<str>, LineEnding),
    Modify(Arc<str>, LineEnding),
    Delete,
}

/// Identity service behind [`Vfs`].
///
/// A single [`FileId`] can be reached through several path spellings. The
/// primary path preserves the first spelling for legacy callers, while aliases
/// record every proven spelling for identity-aware consumers such as file-set
/// partitioning. Every ingress path is registered, including paths that resolve
/// to an existing [`FileId`], so later loader passes can add canonical or
/// filesystem identity evidence for files that were opened before they existed
/// on disk.
#[derive(Default)]
struct VfsIdentityIndex {
    primary_paths: Vec<VfsPath>,
    aliases: Vec<Vec<VfsPath>>,
    exact_paths: FxHashMap<VfsPath, FileId>,
    real_path_keys: FxHashMap<PathKey, FileId>,
    real_paths: FxHashMap<FileIdentityKey, FileId>,
    redirects: Vec<FileId>,
}

/// Result of registering a path ingress with the VFS identity service.
///
/// `file_id` is the canonical owner after all alias evidence has been applied.
/// `merged` contains duplicate ids that were redirected to that owner and whose
/// file state still needs to be reconciled by [`Vfs`].
struct IdentityRegistration {
    file_id: FileId,
    merged: Vec<FileId>,
}

impl VfsIdentityIndex {
    fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        if let Some(file_id) = self.exact_paths.get(path).copied() {
            return Some(self.canonical_file_id(file_id));
        }

        let path = path.as_abs_path()?;
        if let Some(file_id) = self.real_path_keys.get(&PathKey::from_abs_path(path)).copied() {
            return Some(self.canonical_file_id(file_id));
        }
        for alias in path_alias_paths(path).into_iter().map(VfsPath::from) {
            if let Some(file_id) = self.exact_paths.get(&alias).copied() {
                return Some(self.canonical_file_id(file_id));
            }
        }

        let identity = FileIdentityKey::from_path(path)?;
        self.real_paths.get(&identity).copied().map(|file_id| self.canonical_file_id(file_id))
    }

    fn file_path(&self, file_id: FileId) -> Option<&VfsPath> {
        let file_id = self.canonical_file_id(file_id);
        self.primary_paths.get(file_id.0 as usize)
    }

    fn original_file_path(&self, file_id: FileId) -> Option<&VfsPath> {
        self.primary_paths.get(file_id.0 as usize)
    }

    fn file_paths(&self, file_id: FileId) -> &[VfsPath] {
        let file_id = self.canonical_file_id(file_id);
        self.aliases.get(file_id.0 as usize).map_or(&[], Vec::as_slice)
    }

    /// Resolves or allocates a file id and records the current path as alias
    /// evidence for that id.
    fn file_id_or_alloc(&mut self, path: &VfsPath) -> IdentityRegistration {
        let file_id = self.file_id(path).unwrap_or_else(|| self.alloc_file_id(path));
        self.register_path(path, file_id)
    }

    fn len(&self) -> usize {
        self.primary_paths.len()
    }

    fn canonical_file_id(&self, file_id: FileId) -> FileId {
        let mut current = file_id;
        while let Some(next) = self.redirects.get(current.0 as usize).copied()
            && next != current
        {
            current = next;
        }
        current
    }

    fn alloc_file_id(&mut self, path: &VfsPath) -> FileId {
        let id = self.primary_paths.len();
        assert!(id < u32::MAX as usize);
        let file_id = FileId(id as u32);
        self.primary_paths.push(path.clone());
        self.aliases.push(Vec::new());
        self.redirects.push(file_id);
        file_id
    }

    fn register_path(&mut self, path: &VfsPath, file_id: FileId) -> IdentityRegistration {
        let mut owner = self.canonical_file_id(file_id);
        let mut merged = Vec::new();

        if let Some(path) = path.as_abs_path() {
            let aliases = path_alias_paths(path).into_iter().map(VfsPath::from).collect::<Vec<_>>();
            for alias in &aliases {
                if let Some(existing) = self.exact_paths.get(alias).copied() {
                    owner = self.merge_file_ids(owner, existing, &mut merged);
                }
                if let Some(path) = alias.as_abs_path()
                    && let Some(existing) =
                        self.real_path_keys.get(&PathKey::from_abs_path(path)).copied()
                {
                    owner = self.merge_file_ids(owner, existing, &mut merged);
                }
            }
            if let Some(identity) = FileIdentityKey::from_path(path)
                && let Some(existing) = self.real_paths.get(&identity).copied()
            {
                owner = self.merge_file_ids(owner, existing, &mut merged);
            }

            for alias in aliases {
                self.register_exact_path(alias, owner);
            }
            if let Some(identity) = FileIdentityKey::from_path(path) {
                self.real_paths.insert(identity, owner);
            }
        } else {
            if let Some(existing) = self.exact_paths.get(path).copied() {
                owner = self.merge_file_ids(owner, existing, &mut merged);
            }
            self.register_exact_path(path.clone(), owner);
        }

        IdentityRegistration { file_id: owner, merged }
    }

    fn register_exact_path(&mut self, path: VfsPath, file_id: FileId) {
        let file_id = self.canonical_file_id(file_id);
        if let Some(path) = path.as_abs_path() {
            self.real_path_keys.insert(PathKey::from_abs_path(path), file_id);
        }
        self.exact_paths.insert(path.clone(), file_id);

        let Some(aliases) = self.aliases.get_mut(file_id.0 as usize) else {
            return;
        };
        if !aliases.contains(&path) {
            aliases.push(path);
        }
    }

    fn merge_file_ids(&mut self, lhs: FileId, rhs: FileId, merged: &mut Vec<FileId>) -> FileId {
        let lhs = self.canonical_file_id(lhs);
        let rhs = self.canonical_file_id(rhs);
        if lhs == rhs {
            return lhs;
        }

        let (owner, duplicate) = if lhs <= rhs { (lhs, rhs) } else { (rhs, lhs) };
        self.redirects[duplicate.0 as usize] = owner;
        merged.push(duplicate);

        let duplicate_aliases = std::mem::take(&mut self.aliases[duplicate.0 as usize]);
        for alias in duplicate_aliases {
            self.register_exact_path(alias, owner);
        }
        for file_id in self.real_paths.values_mut() {
            if *file_id == duplicate {
                *file_id = owner;
            }
        }
        for file_id in self.real_path_keys.values_mut() {
            if *file_id == duplicate {
                *file_id = owner;
            }
        }

        owner
    }
}

impl Vfs {
    pub(crate) fn file_paths(&self, file_id: FileId) -> &[VfsPath] {
        self.identities.file_paths(file_id)
    }
}

impl Vfs {
    pub fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.identities.file_id(path).filter(|id| self.exists(*id))
    }

    pub fn file_path(&self, file_id: FileId) -> Option<&VfsPath> {
        self.identities.file_path(file_id)
    }

    /// Registers a path at the VFS identity boundary without changing file
    /// contents.
    ///
    /// LSP open notifications use this to learn whether a URI aliases an
    /// already-open analysis buffer before deciding whether the incoming text
    /// should become the canonical VFS text.
    pub fn register_file_ingress(&mut self, path: &VfsPath) -> FileId {
        self.file_id_or_alloc(path)
    }

    /// Returns the first path spelling recorded for this exact [`FileId`].
    ///
    /// Unlike [`Self::file_path`], this does not follow identity redirects.
    /// It is intended for cleanup of historical changes, for example clearing
    /// diagnostics that were published before a duplicate id was merged.
    pub fn original_file_path(&self, file_id: FileId) -> Option<&VfsPath> {
        self.identities.original_file_path(file_id)
    }

    pub fn canonical_file_id(&self, file_id: FileId) -> FileId {
        self.identities.canonical_file_id(file_id)
    }

    pub fn line_ending(&self, file_id: FileId) -> Option<LineEnding> {
        match self.file_state(file_id)? {
            FileState::Exists(_, ending) => Some(*ending),
            FileState::Deleted => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &VfsPath)> + '_ {
        (0..self.file_states.len())
            .map(|it| FileId(it as u32))
            .filter(move |&file_id| self.exists(file_id))
            .filter_map(move |file_id| self.file_path(file_id).map(|path| (file_id, path)))
    }

    pub fn set_file_contents(&mut self, path: &VfsPath, contents: LoadResult) {
        let file_id = self.file_id_or_alloc(path);
        use FileState::*;
        use LoadResult::*;
        let Some(state) = self.file_states.get_mut(file_id.0 as usize) else {
            return;
        };
        let change_kind = match contents {
            Loaded(new, new_ending) => match state {
                Exists(old, _) if *old == new => return,
                Exists(_, _) => {
                    let change_kind = ChangeKind::Modify(Arc::from(new.as_str()), new_ending);
                    *state = Exists(new, new_ending);
                    change_kind
                }
                Deleted => {
                    let change_kind = ChangeKind::Create(Arc::from(new.as_str()), new_ending);
                    *state = Exists(new, new_ending);
                    change_kind
                }
            },
            LoadError => match state {
                Exists(_, _) => {
                    *state = Deleted;
                    ChangeKind::Delete
                }
                Deleted => return,
            },
            DecodeError => return,
        };

        let changed_file = ChangedFile { file_id, change_kind };
        self.changes.push(changed_file);
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn take_changes(&mut self) -> Vec<ChangedFile> {
        mem::take(&mut self.changes)
    }

    pub fn exists(&self, file_id: FileId) -> bool {
        matches!(self.file_state(file_id), Some(FileState::Exists(_, _)))
    }

    fn file_id_or_alloc(&mut self, path: &VfsPath) -> FileId {
        let registration = self.identities.file_id_or_alloc(path);
        let file_id = registration.file_id;
        let id = file_id.0 as usize;
        let len = self.file_states.len().max(id + 1);
        self.file_states.resize(len, FileState::Deleted);
        for duplicate in registration.merged {
            self.merge_file_state(file_id, duplicate);
        }
        file_id
    }

    fn merge_file_state(&mut self, owner: FileId, duplicate: FileId) {
        if owner == duplicate {
            return;
        }

        let owner_idx = owner.0 as usize;
        let duplicate_idx = duplicate.0 as usize;
        let Some(duplicate_state) = self.file_states.get(duplicate_idx).cloned() else {
            return;
        };

        if matches!(self.file_states.get(owner_idx), Some(FileState::Deleted))
            && let FileState::Exists(text, ending) = duplicate_state.clone()
        {
            self.file_states[owner_idx] = FileState::Exists(text.clone(), ending);
            self.changes.push(ChangedFile {
                file_id: owner,
                change_kind: ChangeKind::Create(Arc::from(text.as_str()), ending),
            });
        }

        if matches!(duplicate_state, FileState::Exists(_, _)) {
            self.file_states[duplicate_idx] = FileState::Deleted;
            self.changes.push(ChangedFile { file_id: duplicate, change_kind: ChangeKind::Delete });
        }
    }

    fn file_state(&self, file_id: FileId) -> Option<&FileState> {
        self.file_states.get(file_id.0 as usize)
    }
}

impl fmt::Debug for Vfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vfs").field("n_files", &self.identities.len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use utils::lines::LineEnding;
    #[cfg(windows)]
    use utils::paths::AbsPathBuf;

    use super::*;
    use crate::test_support::TestDir;

    #[test]
    fn load_error_marks_existing_file_deleted() {
        let mut vfs = Vfs::default();
        let path = VfsPath::new_virtual_path("/workspace/vizsla.toml".to_owned());

        vfs.set_file_contents(
            &path,
            LoadResult::Loaded("sources = []\n".to_owned(), LineEnding::Unix),
        );
        let file_id = vfs.file_id(&path).unwrap();
        vfs.take_changes();

        vfs.set_file_contents(&path, LoadResult::LoadError);

        assert!(!vfs.exists(file_id));
        assert_eq!(vfs.file_id(&path), None);
        let changes = vfs.take_changes();
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0].change_kind, ChangeKind::Delete));
    }

    #[cfg(windows)]
    #[test]
    fn real_paths_with_different_drive_letter_case_share_file_id() {
        let path = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap()).join("top.sv");
        let mut lower_drive_path = path.to_string();
        assert_eq!(lower_drive_path.as_bytes().get(1), Some(&b':'));
        lower_drive_path.replace_range(0..1, &lower_drive_path[0..1].to_ascii_lowercase());
        let lower_drive_path = AbsPathBuf::try_from(lower_drive_path.as_str()).unwrap();

        let mut vfs = Vfs::default();
        let upper_vfs_path = VfsPath::from(path);
        let lower_vfs_path = VfsPath::from(lower_drive_path);

        vfs.set_file_contents(
            &upper_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let first_file_id = vfs.file_id(&upper_vfs_path).unwrap();

        vfs.set_file_contents(
            &lower_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let second_file_id = vfs.file_id(&lower_vfs_path).unwrap();

        assert_eq!(first_file_id, second_file_id);
        assert_eq!(vfs.iter().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn real_paths_with_verbatim_and_normal_spelling_share_file_id() {
        let path = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap()).join("top.sv");
        let verbatim_path = AbsPathBuf::try_from(format!(r"\\?\{path}").as_str()).unwrap();

        let mut vfs = Vfs::default();
        let normal_vfs_path = VfsPath::from(path);
        let verbatim_vfs_path = VfsPath::from(verbatim_path);

        vfs.set_file_contents(
            &normal_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let first_file_id = vfs.file_id(&normal_vfs_path).unwrap();

        vfs.set_file_contents(
            &verbatim_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let second_file_id = vfs.file_id(&verbatim_vfs_path).unwrap();

        assert_eq!(first_file_id, second_file_id);
        assert_eq!(vfs.iter().count(), 1);
    }

    #[test]
    fn real_identity_is_registered_when_missing_file_is_created() {
        let dir = TestDir::new("created-identity");
        let source = dir.join("workspace/top.sv");
        let alias = dir.join("alias/top.sv");
        let source_vfs_path = VfsPath::from(source.clone());
        let alias_vfs_path = VfsPath::from(alias.clone());
        let mut vfs = Vfs::default();

        vfs.set_file_contents(
            &source_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let file_id = vfs.file_id(&source_vfs_path).unwrap();

        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = alias.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&source, "module top; endmodule\n").unwrap();
        fs::hard_link(&source, &alias).unwrap();

        vfs.set_file_contents(
            &source_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );

        assert_eq!(vfs.file_id(&alias_vfs_path), Some(file_id));
    }

    #[test]
    fn real_identity_conflict_merges_previously_split_file_ids() {
        let dir = TestDir::new("identity-conflict");
        let source = dir.join("workspace/top.sv");
        let alias = dir.join("alias/top.sv");
        let source_vfs_path = VfsPath::from(source.clone());
        let alias_vfs_path = VfsPath::from(alias.clone());
        let mut vfs = Vfs::default();

        vfs.set_file_contents(
            &source_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let source_file_id = vfs.file_id(&source_vfs_path).unwrap();
        vfs.set_file_contents(
            &alias_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        let alias_file_id = vfs.file_id(&alias_vfs_path).unwrap();
        assert_ne!(source_file_id, alias_file_id);
        vfs.take_changes();

        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = alias.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&source, "module top; endmodule\n").unwrap();
        fs::hard_link(&source, &alias).unwrap();

        vfs.set_file_contents(
            &source_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        vfs.set_file_contents(
            &alias_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );

        let merged_file_id = vfs.file_id(&alias_vfs_path).unwrap();
        assert_eq!(merged_file_id, source_file_id);
        assert_eq!(vfs.file_id(&source_vfs_path), Some(source_file_id));
        assert_eq!(vfs.iter().count(), 1);
        assert!(!vfs.exists(alias_file_id));
        let changes = vfs.take_changes();
        assert!(changes.iter().any(|change| change.file_id == alias_file_id
            && matches!(change.change_kind, ChangeKind::Delete)));
    }
}
