use std::fmt;

use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::{
    get::Get,
    line_index::{TextRange, TextSize},
    paths::{AbsPathBuf, Utf8PathBuf},
};
use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

use super::*;
use crate::{
    base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{
            CompilationProfile, CompilationProfileId, Predefine, PredefineSource, PreprocessConfig,
            ProjectConfig,
        },
        salsa::{self, Durability},
        source_db::{
            FileLoader, PreprocExpansionSourceBuffer, PreprocVirtualOrigin, SourceDb,
            SourceDbStorage, SourceFileKind, SourceRootDb, SourceRootDbStorage,
        },
        source_root::{SourceRoot, SourceRootId},
    },
    container::InFile,
    db::{HirDb, HirDbStorage, InternDbStorage},
    hir_def::module::ModuleId,
    source_map::IsSrc,
};

const TOP: FileId = FileId(0);
const HEADER: FileId = FileId(1);
const LEAF: FileId = FileId(2);
const MANIFEST: FileId = FileId(3);
const ROOT: SourceRootId = SourceRootId(0);
const PROFILE: CompilationProfileId = CompilationProfileId(0);

#[salsa::database(SourceDbStorage, SourceRootDbStorage, InternDbStorage, HirDbStorage)]
#[derive(Default)]
struct TestDb {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for TestDb {}

impl fmt::Debug for TestDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestDb").finish()
    }
}

impl FileLoader for TestDb {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
        SourceRootDb::source_root(self, source_root_id).resolve_path(path)
    }
}

fn db_with_files(root_text: &str, header_text: &str) -> TestDb {
    db_with_entries(&[(TOP, "rtl/top.v", root_text), (HEADER, "include/defs.vh", header_text)])
}

fn db_with_nested_files(root_text: &str, header_text: &str, leaf_text: &str) -> TestDb {
    db_with_entries(&[
        (TOP, "rtl/top.v", root_text),
        (HEADER, "include/defs.vh", header_text),
        (LEAF, "include/leaf.vh", leaf_text),
    ])
}

fn db_with_entries(entries: &[(FileId, &str, &str)]) -> TestDb {
    db_with_entries_and_predefines(entries, Vec::new())
}

fn db_with_entries_and_predefines(
    entries: &[(FileId, &str, &str)],
    predefines: Vec<String>,
) -> TestDb {
    db_with_entries_and_predefine_entries(
        entries,
        predefines.into_iter().map(Predefine::new).collect(),
    )
}

fn db_with_entries_and_predefine_entries(
    entries: &[(FileId, &str, &str)],
    predefines: Vec<Predefine>,
) -> TestDb {
    let include_dir = abs_path("include");

    let mut file_set = FileSet::default();
    for (file_id, path, _) in entries {
        file_set.insert(*file_id, VfsPath::from(abs_path(path)));
    }
    let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);

    let preprocess = PreprocessConfig { predefines, include_dirs: vec![include_dir.clone()] };
    let project_config = ProjectConfig::new(
        vec![Some(PROFILE)],
        vec![CompilationProfile {
            source_roots: vec![ROOT],
            top_modules: Vec::new(),
            preprocess: preprocess.clone(),
        }],
    );

    let mut files = FxHashSet::default();
    for (file_id, _, _) in entries {
        files.insert(*file_id);
    }

    let mut db = TestDb::default();
    db.set_files_with_durability(Box::new(files), Durability::HIGH);
    db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
    db.set_diagnostics_config_with_durability(
        Arc::new(DiagnosticsConfig::default()),
        Durability::HIGH,
    );
    db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);

    for (file_id, path, text) in entries {
        let path = abs_path(path);
        let vfs_path = VfsPath::from(path.clone());
        db.set_source_root_id_with_durability(*file_id, ROOT, Durability::LOW);
        db.set_file_path_with_durability(*file_id, Some(path), Durability::LOW);
        db.set_file_kind_with_durability(
            *file_id,
            SourceFileKind::from_path(&vfs_path),
            Durability::LOW,
        );
        db.set_file_text_with_durability(*file_id, Arc::from(*text), Durability::LOW);
        db.set_file_preprocess_config_with_durability(
            *file_id,
            Arc::new(preprocess.clone()),
            Durability::LOW,
        );
    }

    db
}

fn abs_path(path: &str) -> AbsPathBuf {
    let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
    AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
}

fn offset(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
}

fn offset_after(text: &str, needle: &str) -> TextSize {
    TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
}

fn offset_after_n(text: &str, needle: &str, occurrence: usize) -> TextSize {
    let mut cursor = 0;
    for index in 0..=occurrence {
        let relative = text[cursor..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?} in fixture"));
        let absolute = cursor + relative;
        if index == occurrence {
            return TextSize::from(u32::try_from(absolute + needle.len()).unwrap());
        }
        cursor = absolute + needle.len();
    }
    unreachable!()
}

fn text_at_range(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn assert_expansion_is_display_only_source_buffer(
    mapped: &MappedSourcePreprocModel,
    expansion: &MacroExpansion,
) {
    let expansion_id = SourceMacroExpansionId::new(expansion.id.raw());
    let entry =
        mapped.source_map.expansion(expansion_id).expect("expansion should have a display entry");
    assert!(matches!(&entry.source_buffer, PreprocExpansionSourceBuffer::DisplayOnly { .. }));
    assert!(matches!(
        mapped.source_map.emitted_source_buffer_range(expansion_id, expansion.emitted_token_range),
        Err(PreprocSourceMapError::DisplayOnlyVirtualSource { .. })
    ));
}

mod diagnostics;
mod expansion_display;
mod expansion_query;
mod include_context;
mod manifest;
mod reference_context;
mod reference_index;
