use std::{fmt, path::PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};
use salsa::{Durability, Setter};
use triomphe::Arc;
use vfs::{AnchoredPath, FileId};
pub use workspace_model::source_db::SourceFileKind;

use crate::{
    diagnostics_config::DiagnosticsConfig,
    project::{CompilationProfileId, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};

pub trait FileLoader {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

/// Values retained by Salsa must either implement `SalsaValue` or be known not
/// to contain database-lifetime references. These wrappers keep the low-level
/// workspace model independent of Salsa while making that invariant explicit.
#[derive(Clone, Copy)]
struct SourceFileKindValue(SourceFileKind);

// SAFETY: `SourceFileKind` is a copy-only, `'static` enum with no database
// references.
unsafe impl salsa::SalsaValue for SourceFileKindValue {}

#[derive(Clone)]
struct SourceRootValue(Arc<SourceRoot>);

// SAFETY: `SourceRoot` is owned data with no database-lifetime references.
unsafe impl salsa::SalsaValue for SourceRootValue {}

#[derive(Clone)]
struct DiagnosticsConfigValue(Arc<DiagnosticsConfig>);

// SAFETY: `DiagnosticsConfig` is owned data with no database-lifetime
// references.
unsafe impl salsa::SalsaValue for DiagnosticsConfigValue {}

#[derive(Clone)]
struct ProjectConfigValue(Arc<ProjectConfig>);

// SAFETY: `ProjectConfig` is owned data with no database-lifetime references.
unsafe impl salsa::SalsaValue for ProjectConfigValue {}

#[salsa::input]
struct SourceFile {
    #[returns(clone)]
    text: Arc<str>,
    #[returns(copy)]
    kind: SourceFileKindValue,
    #[returns(clone)]
    path: Option<PathBuf>,
}

#[salsa::input(singleton)]
struct SourceFiles {
    #[returns(clone)]
    files: FxHashMap<u32, SourceFile>,
}

#[salsa::input(singleton)]
struct SourceRoots {
    #[returns(clone)]
    roots: FxHashMap<u32, SourceRootValue>,
    #[returns(clone)]
    file_root_ids: FxHashMap<u32, u32>,
}

#[salsa::input(singleton)]
struct ProjectInputs {
    #[returns(clone)]
    diagnostics_config: DiagnosticsConfigValue,
    #[returns(clone)]
    project_config: ProjectConfigValue,
}

/// Ground-state source files and project configuration.
///
/// Derived preprocessing, lowering, and type-system queries belong to higher
/// database layers.
#[salsa::db]
pub trait SourceDb: salsa::Database + FileLoader + fmt::Debug {
    fn file_text(&self, file_id: FileId) -> Arc<str> {
        source_file(self, file_id).text(self)
    }

    fn file_kind(&self, file_id: FileId) -> SourceFileKind {
        source_file(self, file_id).kind(self).0
    }

    fn file_path(&self, file_id: FileId) -> Option<utils::paths::AbsPathBuf> {
        source_file(self, file_id).path(self).map(|path| {
            utils::paths::abs_path_buf_from_path_buf(path)
                .expect("source file path must be absolute and UTF-8")
        })
    }

    fn files(&self) -> FxHashSet<FileId> {
        let registry = SourceFiles::get(self);
        registry.files(self).keys().copied().map(FileId::from_raw).collect()
    }

    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig> {
        ProjectInputs::get(self).diagnostics_config(self).0
    }

    fn project_config(&self) -> Arc<ProjectConfig> {
        ProjectInputs::get(self).project_config(self).0
    }

    fn set_file_text_with_durability(
        &mut self,
        file_id: FileId,
        text: Arc<str>,
        durability: Durability,
    ) {
        source_file_mut(self, file_id).set_text(self).with_durability(durability).to(text);
    }

    fn set_file_kind_with_durability(
        &mut self,
        file_id: FileId,
        kind: SourceFileKind,
        durability: Durability,
    ) {
        source_file_mut(self, file_id)
            .set_kind(self)
            .with_durability(durability)
            .to(SourceFileKindValue(kind));
    }

    fn set_file_path_with_durability(
        &mut self,
        file_id: FileId,
        path: Option<utils::paths::AbsPathBuf>,
        durability: Durability,
    ) {
        source_file_mut(self, file_id)
            .set_path(self)
            .with_durability(durability)
            .to(path.map(Into::into));
    }

    fn set_files_with_durability(&mut self, files: FxHashSet<FileId>, durability: Durability) {
        let registry = ensure_source_files(self);
        let mut file_inputs = registry.files(self).clone();
        file_inputs.retain(|file_id, _| files.contains(&FileId::from_raw(*file_id)));
        for file_id in files.iter().copied() {
            file_inputs.entry(file_id.index()).or_insert_with(|| {
                SourceFile::new(
                    self,
                    Arc::from(""),
                    SourceFileKindValue(SourceFileKind::default()),
                    None,
                )
            });
        }
        registry.set_files(self).with_durability(durability).to(file_inputs);
    }

    fn set_diagnostics_config_with_durability(
        &mut self,
        config: Arc<DiagnosticsConfig>,
        durability: Durability,
    ) {
        let inputs = ensure_project_inputs(self);
        inputs
            .set_diagnostics_config(self)
            .with_durability(durability)
            .to(DiagnosticsConfigValue(config));
    }

    fn set_project_config_with_durability(
        &mut self,
        config: Arc<ProjectConfig>,
        durability: Durability,
    ) {
        let inputs = ensure_project_inputs(self);
        inputs.set_project_config(self).with_durability(durability).to(ProjectConfigValue(config));
    }
}

#[salsa::db]
pub trait SourceRootDb: SourceDb {
    fn source_root_id(&self, file_id: FileId) -> SourceRootId {
        let registry = SourceRoots::get(self);
        SourceRootId(
            *registry
                .file_root_ids(self)
                .get(&file_id.index())
                .unwrap_or_else(|| panic!("missing source root for file {file_id:?}")),
        )
    }

    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot> {
        let registry = SourceRoots::get(self);
        registry
            .roots(self)
            .get(&id.0)
            .unwrap_or_else(|| panic!("missing source root {id:?}"))
            .0
            .clone()
    }

    fn set_source_root_id_with_durability(
        &mut self,
        file_id: FileId,
        source_root_id: SourceRootId,
        durability: Durability,
    ) {
        let registry = ensure_source_roots(self);
        let mut file_root_ids = registry.file_root_ids(self).clone();
        file_root_ids.insert(file_id.index(), source_root_id.0);
        registry.set_file_root_ids(self).with_durability(durability).to(file_root_ids);
    }

    fn set_source_root_with_durability(
        &mut self,
        source_root_id: SourceRootId,
        source_root: Arc<SourceRoot>,
        durability: Durability,
    ) {
        let registry = ensure_source_roots(self);
        let mut roots = registry.roots(self).clone();
        roots.insert(source_root_id.0, SourceRootValue(source_root));
        registry.set_roots(self).with_durability(durability).to(roots);
    }

    fn file_compilation_profile(&self, file_id: FileId) -> Option<CompilationProfileId> {
        let source_root_id = self.source_root_id(file_id);
        let project_config = self.project_config();
        let profile_id = project_config.profile_for_root(source_root_id);
        let source_root = self.source_root(source_root_id);
        if profile_id.is_none() && source_root.role().reports_missing_profile() {
            tracing::debug!(
                ?file_id,
                ?source_root_id,
                root_profile_count = project_config.root_profile_count(),
                "file has no compilation profile",
            );
        }
        profile_id
    }

    fn file_is_project_ignored(&self, file_id: FileId) -> bool {
        let source_root_id = self.source_root_id(file_id);
        self.source_root(source_root_id).is_ignored()
    }
}

fn source_file<Db: SourceDb + ?Sized>(db: &Db, file_id: FileId) -> SourceFile {
    SourceFiles::get(db)
        .files(db)
        .get(&file_id.index())
        .copied()
        .unwrap_or_else(|| panic!("missing source file input for {file_id:?}"))
}

fn source_file_mut<Db: SourceDb + ?Sized>(db: &mut Db, file_id: FileId) -> SourceFile {
    let registry = ensure_source_files(db);
    if let Some(file) = registry.files(db).get(&file_id.index()).copied() {
        return file;
    }

    let file =
        SourceFile::new(db, Arc::from(""), SourceFileKindValue(SourceFileKind::default()), None);
    let mut files = registry.files(db).clone();
    files.insert(file_id.index(), file);
    registry.set_files(db).to(files);
    file
}

fn ensure_source_files<Db: SourceDb + ?Sized>(db: &mut Db) -> SourceFiles {
    SourceFiles::try_get(db).unwrap_or_else(|| SourceFiles::new(db, FxHashMap::default()))
}

fn ensure_source_roots<Db: SourceRootDb + ?Sized>(db: &mut Db) -> SourceRoots {
    SourceRoots::try_get(db)
        .unwrap_or_else(|| SourceRoots::new(db, FxHashMap::default(), FxHashMap::default()))
}

fn ensure_project_inputs<Db: SourceDb + ?Sized>(db: &mut Db) -> ProjectInputs {
    ProjectInputs::try_get(db).unwrap_or_else(|| {
        ProjectInputs::new(
            db,
            DiagnosticsConfigValue(Arc::new(DiagnosticsConfig::default())),
            ProjectConfigValue(Arc::new(ProjectConfig::default())),
        )
    })
}
