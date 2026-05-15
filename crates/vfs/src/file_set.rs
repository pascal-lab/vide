use fst::{IntoStreamer, Streamer};
use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;

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
        let mut scratch_space = Vec::new();
        let mut set = vec![FileSet::default(); self.len];
        for (file_id, path) in vfs.iter() {
            let root = self.classify(path, &mut scratch_space);
            if let Some(file_set) = set.get_mut(root) {
                file_set.insert(file_id, path.clone());
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
            longest_prefix = v as usize;
        }
        longest_prefix
    }
}

/// Builder for [`FileSetConfig`].
#[derive(Default)]
pub struct FileSetConfigBuilder {
    roots: Vec<Vec<VfsPath>>,
}

impl FileSetConfigBuilder {
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn add_file_set(&mut self, roots: Vec<VfsPath>) {
        self.roots.push(roots);
    }

    pub fn build(self) -> FileSetConfig {
        let len = self.roots.len() + 1;
        let mut entries = self
            .roots
            .into_iter()
            .enumerate()
            .flat_map(|(i, paths)| {
                paths.into_iter().map(move |p| {
                    let mut buf = Vec::new();
                    p.encode(&mut buf);
                    (buf, i as u64)
                })
            })
            .collect::<Vec<_>>();

        // make sure that the longer one comes later
        entries.sort();
        entries.dedup_by(|(a, _), (b, _)| a == b);

        FileSetConfig { len, map: fst::Map::from_iter(entries).unwrap_or_default() }
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
