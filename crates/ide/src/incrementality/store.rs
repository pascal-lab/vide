use base_db::source_db::SourceDb;
use design_graph::{GeneratedUnits, UnitId, UnitMeta};
use parking_lot::Mutex;
use preproc_expand::db::PreprocDb;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::db::root_db::RootDb;

#[derive(Clone, Default)]
struct Inner {
    /// Authoritative standalone parses retained by this store lineage:
    /// compilation root -> files named by emitted preprocessor include edges.
    parse_dependencies: FxHashMap<FileId, Arc<[FileId]>>,
    /// Generated CU units from paid artifacts, keyed by artifact fingerprint.
    generated: GeneratedUnits,
}

/// Overlay and parse-dependency book-keeping, forked on every change so
/// previously created [`crate::analysis::AnalysisSnapshot`]s keep the previous
/// overlay and can never observe generated names from a later edit.
///
/// Source catalogs live in salsa. This store does not memoize them.
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
    /// One revision transition. Fork the overlay, apply the salsa change,
    /// drop generated entries whose artifact fingerprint no longer matches.
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
        store.drop_stale_generated(db);
        (triomphe::Arc::new(store), affected_files)
    }

    pub(crate) fn fork(&self) -> Self {
        Self { inner: Mutex::new(self.inner.lock().clone()) }
    }

    pub(crate) fn record_parse_dependencies(&self, file_id: FileId, dependencies: Arc<[FileId]>) {
        self.inner.lock().parse_dependencies.insert(file_id, dependencies);
    }

    /// Book-keep generated units for one paid artifact. `fingerprint` is
    /// [`PreprocDb::compilation_unit_snapshot`]; a later snapshot with a
    /// different fingerprint cannot observe this entry.
    pub(crate) fn record_generated_units(
        &self,
        file_id: FileId,
        fingerprint: u64,
        ids: Box<[UnitId]>,
        meta: FxHashMap<UnitId, UnitMeta>,
    ) -> bool {
        self.inner.lock().generated.replace_file(file_id, fingerprint, ids, meta)
    }

    /// Generated units whose stored fingerprint still matches the current
    /// compilation-unit snapshot. Stale entries are a miss, not a value.
    pub(crate) fn generated_units(&self, db: &RootDb) -> GeneratedUnits {
        let mut generated = self.inner.lock().generated.clone();
        generated.retain_current(|file, fingerprint| {
            db.files().contains(&file)
                && <dyn PreprocDb>::compilation_unit_snapshot(db, file).fingerprint == fingerprint
        });
        generated
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

    fn drop_stale_generated(&self, db: &RootDb) {
        let files: Vec<FileId> = self.inner.lock().generated.by_file.keys().copied().collect();
        let current: FxHashMap<FileId, u64> = files
            .into_iter()
            .filter(|&file| db.files().contains(&file))
            .map(|file| (file, <dyn PreprocDb>::compilation_unit_snapshot(db, file).fingerprint))
            .collect();
        self.inner.lock().generated.retain_current(|file, fingerprint| {
            current.get(&file).is_some_and(|&got| got == fingerprint)
        });
    }
}
