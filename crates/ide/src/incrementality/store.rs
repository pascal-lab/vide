use base_db::source_db::SourceDb;
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::db::root_db::RootDb;

#[derive(Clone, Default)]
struct Inner {
    /// Authoritative standalone parses retained by this store lineage:
    /// compilation root -> files named by emitted preprocessor include edges.
    parse_dependencies: FxHashMap<FileId, Arc<[FileId]>>,
}

/// Parse-dependency book-keeping, forked on every change so previously
/// created [`crate::analysis::AnalysisSnapshot`]s keep the previous paid-file
/// set. Source catalogs live in salsa. This store does not memoize them.
///
/// Owned by [`crate::analysis_host::AnalysisHost`].
#[derive(Default)]
pub(crate) struct ProductStore {
    inner: Mutex<Inner>,
}

impl std::panic::RefUnwindSafe for ProductStore {}
impl std::panic::UnwindSafe for ProductStore {}

impl std::fmt::Debug for ProductStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductStore").finish()
    }
}

impl ProductStore {
    /// One revision transition. Fork parse-deps, apply the salsa change.
    pub(crate) fn transition(
        current: &triomphe::Arc<Self>,
        db: &mut RootDb,
        change: base_db::change::Change,
    ) -> (triomphe::Arc<Self>, Vec<FileId>) {
        let dirty_files: Vec<_> = change.changed_files.iter().map(|file| file.file_id).collect();
        if change.project_config.is_some() {
            db.apply_change(change);
            let files = db.files().iter().copied().collect();
            return (triomphe::Arc::new(Self::default()), files);
        }
        let dependent_files = current.parsed_dependents(&dirty_files);
        let mut affected_files = dirty_files;
        affected_files.extend(dependent_files);
        affected_files.sort_unstable_by_key(|file| file.index());
        affected_files.dedup();
        if affected_files.is_empty() {
            db.apply_change(change);
            return (current.clone(), Vec::new());
        }
        let store = current.fork();
        db.apply_change(change);
        (triomphe::Arc::new(store), affected_files)
    }

    pub(crate) fn fork(&self) -> Self {
        Self { inner: Mutex::new(self.inner.lock().clone()) }
    }

    pub(crate) fn record_parse_dependencies(&self, file_id: FileId, dependencies: Arc<[FileId]>) {
        self.inner.lock().parse_dependencies.insert(file_id, dependencies);
    }

    pub(crate) fn record_paid_file(&self, file_id: FileId) {
        self.inner
            .lock()
            .parse_dependencies
            .entry(file_id)
            .or_insert_with(|| Arc::from(Vec::<FileId>::new()));
    }

    /// Files whose paid parse may be consulted for macro-generated owners.
    pub(crate) fn paid_files(&self) -> Vec<FileId> {
        let mut files: Vec<_> = self.inner.lock().parse_dependencies.keys().copied().collect();
        files.sort_by_key(|file| file.index());
        files
    }

    pub(crate) fn parsed_dependents(&self, changed: &[FileId]) -> Vec<FileId> {
        let changed = changed.iter().copied().collect::<FxHashSet<_>>();
        self.inner
            .lock()
            .parse_dependencies
            .iter()
            .filter_map(|(&file_id, dependencies)| {
                (!changed.contains(&file_id)
                    && dependencies.iter().any(|dependency| changed.contains(dependency)))
                .then_some(file_id)
            })
            .collect()
    }
}
