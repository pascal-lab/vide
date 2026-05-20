use fst::{IntoStreamer, Streamer};
use nohash_hasher::IntMap;
use rustc_hash::{FxHashMap, FxHashSet};
use utils::paths::{AbsPath, AbsPathBuf};

use crate::{
    anchored_path::AnchoredPath,
    vfs::{FileId, Vfs},
    vfs_path::VfsPath,
};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct FileSet {
    files: FxHashMap<VfsPath, FileId>,
    paths: IntMap<FileId, VfsPath>,
}

impl FileSet {
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get_file(&self, path: &VfsPath) -> Option<&FileId> {
        self.files.get(path)
    }

    pub fn get_path(&self, file: &FileId) -> Option<&VfsPath> {
        self.paths.get(file)
    }

    pub fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let mut base = self.paths.get(&path.anchor_id)?.clone();
        base.pop();
        let path = base.join(path.path)?;
        self.files.get(&path).copied()
    }

    pub fn insert(&mut self, file_id: FileId, path: VfsPath) {
        self.files.insert(path.clone(), file_id);
        self.paths.insert(file_id, path);
    }

    pub fn iter(&self) -> impl Iterator<Item = FileId> + '_ {
        self.paths.keys().copied()
    }
}

#[derive(Debug)]
pub struct FileSetConfig {
    // Number of sets that can partition into.
    // This should be `self.map.len() + 1` for files that don't fit in any defined set.
    len: usize,
    // Encoded paths -> sets they belong to.
    map: fst::Map<Vec<u8>>,
    filters: Vec<FileSetFilter>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct PartitionedFileSet {
    pub file_set: FileSet,
    pub source_files: Option<FxHashSet<FileId>>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct PathSelection {
    pub roots: Vec<AbsPathBuf>,
}

impl PathSelection {
    pub fn all(roots: Vec<AbsPathBuf>) -> Self {
        Self { roots }
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn contains_file(&self, path: &AbsPath) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }

    pub fn contains_dir(&self, path: &AbsPath) -> bool {
        self.roots.iter().any(|root| path.starts_with(root))
    }

    pub fn roots(&self) -> impl Iterator<Item = &AbsPathBuf> {
        self.roots.iter()
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct FileSetFilter {
    pub include: Vec<PathSelection>,
    pub source: Option<Vec<PathSelection>>,
    pub exclude_paths: Vec<AbsPathBuf>,
}

impl FileSetFilter {
    fn matches(&self, path: &VfsPath) -> bool {
        let Some(path) = path.as_abs_path() else {
            return self.include.is_empty() && self.exclude_paths.is_empty();
        };
        self.include.iter().any(|include| include.contains_file(path))
            && !self.exclude_paths.iter().any(|exclude| path.starts_with(exclude))
    }

    fn is_source(&self, path: &VfsPath) -> bool {
        let Some(source) = &self.source else {
            return false;
        };
        let Some(path) = path.as_abs_path() else {
            return false;
        };
        source.iter().any(|source| source.contains_file(path))
            && !self.exclude_paths.iter().any(|exclude| path.starts_with(exclude))
    }
}

impl Default for FileSetConfig {
    fn default() -> Self {
        FileSetConfig::builder().build()
    }
}

impl FileSetConfig {
    pub fn builder() -> FileSetConfigBuilder {
        FileSetConfigBuilder::default()
    }

    pub fn partition(&self, vfs: &Vfs) -> Vec<FileSet> {
        self.partition_with_source(vfs).into_iter().map(|partition| partition.file_set).collect()
    }

    pub fn partition_with_source(&self, vfs: &Vfs) -> Vec<PartitionedFileSet> {
        let mut scratch_space = Vec::new();
        let mut set = (0..self.len)
            .map(|idx| PartitionedFileSet {
                file_set: FileSet::default(),
                source_files: self
                    .filters
                    .get(idx)
                    .and_then(|filter| filter.source.as_ref().map(|_| FxHashSet::default())),
            })
            .collect::<Vec<_>>();
        for (file_id, path) in vfs.iter() {
            let root = self.classify(path, &mut scratch_space);
            if let Some(partition) = set.get_mut(root) {
                partition.file_set.insert(file_id, path.clone());
                if self.filters.get(root).is_some_and(|filter| filter.is_source(path))
                    && let Some(source_files) = &mut partition.source_files
                {
                    source_files.insert(file_id);
                }
            }
        }
        set
    }

    fn classify(&self, path: &VfsPath, scratch_space: &mut Vec<u8>) -> usize {
        scratch_space.clear();
        path.encode(scratch_space);
        let automaton = PrefixOf::new(scratch_space.as_slice());
        let mut longest_prefix = self.len - 1;
        let mut stream = self.map.search(automaton).into_stream();
        while let Some((_, v)) = stream.next() {
            let idx = v as usize;
            if self.filters.get(idx).is_some_and(|filter| filter.matches(path)) {
                longest_prefix = idx;
            }
        }
        longest_prefix
    }
}

/// Builder for [`FileSetConfig`].
#[derive(Default)]
pub struct FileSetConfigBuilder {
    roots: Vec<FileSetSpec>,
}

struct FileSetSpec {
    roots: Vec<VfsPath>,
    filter: FileSetFilter,
}

impl FileSetConfigBuilder {
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn add_file_set(&mut self, roots: Vec<VfsPath>) {
        let include_paths: Vec<AbsPathBuf> = roots
            .iter()
            .filter_map(|root| root.as_abs_path().map(|path| path.to_path_buf()))
            .collect();
        let include = if include_paths.is_empty() {
            Vec::new()
        } else {
            vec![PathSelection::all(include_paths)]
        };
        self.add_filtered_file_set(roots, FileSetFilter { include, ..FileSetFilter::default() });
    }

    pub fn add_filtered_file_set(&mut self, roots: Vec<VfsPath>, filter: FileSetFilter) {
        self.roots.push(FileSetSpec { roots, filter });
    }

    pub fn build(self) -> FileSetConfig {
        let len = self.roots.len() + 1;
        let filters = self.roots.iter().map(|spec| spec.filter.clone()).collect();
        let mut entries = self
            .roots
            .into_iter()
            .enumerate()
            .flat_map(|(i, spec)| {
                spec.roots.into_iter().map(move |p| {
                    let mut buf = Vec::new();
                    p.encode(&mut buf);
                    (buf, i as u64)
                })
            })
            .collect::<Vec<_>>();

        // make sure that the longer one comes later
        entries.sort();
        entries.dedup_by(|(a, _), (b, _)| a == b);

        FileSetConfig { len, map: fst::Map::from_iter(entries).unwrap_or_default(), filters }
    }
}

// It will match if `prefix_of` is a prefix of the given data.
struct PrefixOf<'a> {
    prefix_of: &'a [u8],
}

impl<'a> PrefixOf<'a> {
    /// Creates a new `PrefixOf` from the given slice.
    fn new(prefix_of: &'a [u8]) -> Self {
        Self { prefix_of }
    }
}

impl fst::Automaton for PrefixOf<'_> {
    type State = usize;

    fn start(&self) -> usize {
        0
    }

    fn is_match(&self, &state: &usize) -> bool {
        state != !0
    }

    fn can_match(&self, &state: &usize) -> bool {
        state != !0
    }

    fn accept(&self, &state: &usize, byte: u8) -> usize {
        if self.prefix_of.get(state) == Some(&byte) { state + 1 } else { !0 }
    }
}
