use triomphe::Arc;
use utils::paths::AbsPathBuf;
use vfs::FileId;

use crate::base_db::project::CompilationProfileId;

/// Stable identity for all analysis results derived from one immutable
/// analysis state.
///
/// The id advances when the analysis host applies a change. Creating several
/// read-only views of the same host state does not create several identities.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnalysisSnapshotId(u64);

impl AnalysisSnapshotId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0.checked_add(1).expect("analysis snapshot id exhausted"))
    }
}

/// The complete compilation inputs visible to a single analysis snapshot.
///
/// Collections are stored behind immutable slices so callers cannot mutate a
/// context in place and accidentally make a result refer to a different
/// compilation. `roots` and `library_maps` contain VFS file ids, not paths;
/// their text and paths are owned by the same salsa snapshot as the context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationContext {
    pub profile: Option<CompilationProfileId>,
    pub roots: Arc<[FileId]>,
    pub include_dirs: Arc<[AbsPathBuf]>,
    pub predefines: Arc<[String]>,
    pub library_maps: Arc<[FileId]>,
    pub top_modules: Arc<[String]>,
}

impl CompilationContext {
    pub fn new(
        profile: Option<CompilationProfileId>,
        roots: impl Into<Arc<[FileId]>>,
        include_dirs: impl Into<Arc<[AbsPathBuf]>>,
        predefines: impl Into<Arc<[String]>>,
        library_maps: impl Into<Arc<[FileId]>>,
        top_modules: impl Into<Arc<[String]>>,
    ) -> Self {
        Self {
            profile,
            roots: roots.into(),
            include_dirs: include_dirs.into(),
            predefines: predefines.into(),
            library_maps: library_maps.into(),
            top_modules: top_modules.into(),
        }
    }

    pub fn root_ids(&self) -> &[FileId] {
        &self.roots
    }

    pub fn library_map_ids(&self) -> &[FileId] {
        &self.library_maps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_ids_are_monotonic() {
        let initial = AnalysisSnapshotId::new(41);
        assert_eq!(initial.get(), 41);
        assert_eq!(initial.next().get(), 42);
        assert_eq!(AnalysisSnapshotId::default().get(), 0);
    }

    #[test]
    #[should_panic(expected = "analysis snapshot id exhausted")]
    fn snapshot_id_overflow_fails_fast() {
        AnalysisSnapshotId::new(u64::MAX).next();
    }

    #[test]
    fn compilation_context_owns_immutable_inputs() {
        let context = CompilationContext::new(
            Some(CompilationProfileId(3)),
            vec![FileId(1)],
            Vec::<AbsPathBuf>::new(),
            vec!["FEATURE=1".to_owned()],
            vec![FileId(2)],
            vec!["top".to_owned()],
        );

        assert_eq!(context.profile, Some(CompilationProfileId(3)));
        assert_eq!(context.root_ids(), [FileId(1)]);
        assert_eq!(context.library_map_ids(), [FileId(2)]);
    }
}
