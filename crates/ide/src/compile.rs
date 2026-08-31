//! File-closure radius and a calling-thread [`compile`].
//!
//! This is the slang door that does not go through [`crate::elaboration`].
//! Hover, goto, `.` / `::`, and types wait on [`Compiler::compile`] of this
//! radius. Profile extras (instances, hierarchy) may still ask the worker.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use base_db::{project::CompilationProfileId, source_db::SourceRootDb};
use design_graph::DesignGraphDb;
use preproc_expand::{compilation_plan, db::PreprocDb};
use rustc_hash::{FxHashMap, FxHashSet};
use slang_sys::compilation::{Compilation, SourceSession};
use syntax::SyntaxTreeOptions;
use vfs::FileId;

use crate::db::root_db::RootDb;

/// Files in the keystroke neighborhood of `F`.
///
/// Contains `F`, compilation units it names, and the include graph of those
/// files. `` `include `` into a package is in the set — that is the product
/// bet versus slang-server's smaller radius.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileClosure {
    files: Vec<FileId>,
}

impl FileClosure {
    pub fn contains(&self, file: FileId) -> bool {
        self.files.contains(&file)
    }

    pub fn files(&self) -> &[FileId] {
        &self.files
    }
}

/// Direct CU names only: `F` plus catalog files named by `F`'s imports /
/// instantiations / `::` heads. No include graph. slang-server's keystroke
/// set; kept so tests can show the include-into-package difference.
pub fn direct_names(db: &RootDb, file: FileId) -> FileClosure {
    named_files(db, file, false)
}

/// `F` plus named compilation units plus the include graph of every file
/// reached. Catalog `files[0]` is never used: every ambiguous name contributes
/// all candidate files.
pub fn file_closure(db: &RootDb, file: FileId) -> FileClosure {
    named_files(db, file, true)
}

fn named_files(db: &RootDb, start: FileId, walk_includes: bool) -> FileClosure {
    let mut seen = FxHashSet::default();
    let mut files = Vec::new();
    let mut pending = Vec::new();
    absorb(db, start, walk_includes, &mut seen, &mut files, &mut pending);

    if db.file_compilation_profile(start).is_some() {
        let catalog = <dyn DesignGraphDb>::source_unit_catalog(db);
        let mut index = 0;
        while index < pending.len() {
            let file = pending[index];
            index += 1;
            let facts = <dyn DesignGraphDb>::file_facts(db, file);
            for import in facts.imports.iter() {
                for unit in catalog.packages_named(&import.package).into_candidates() {
                    absorb(db, unit.file, walk_includes, &mut seen, &mut files, &mut pending);
                }
            }
            for package_ref in facts.package_refs.iter() {
                for unit in catalog.packages_named(&package_ref.name).into_candidates() {
                    absorb(db, unit.file, walk_includes, &mut seen, &mut files, &mut pending);
                }
            }
            for site in facts.instantiations.iter() {
                for unit in catalog.candidates(&site.name, site.role) {
                    absorb(db, unit.file, walk_includes, &mut seen, &mut files, &mut pending);
                }
            }
        }
    }

    files.sort_unstable_by_key(|file| file.index());
    FileClosure { files }
}

fn absorb(
    db: &RootDb,
    file: FileId,
    walk_includes: bool,
    seen: &mut FxHashSet<FileId>,
    files: &mut Vec<FileId>,
    pending: &mut Vec<FileId>,
) {
    if !seen.insert(file) {
        return;
    }
    files.push(file);
    pending.push(file);
    if !walk_includes {
        return;
    }
    for &included in <dyn PreprocDb>::static_include_closure(db, file).files() {
        absorb(db, included, true, seen, files, pending);
    }
}

/// Inputs that vary by radius. Closure and profile pass different file sets
/// into the same [`Compiler::compile`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompileOptions {
    pub predefines: Vec<String>,
    pub include_dirs: Vec<String>,
    pub top_modules: Vec<String>,
}

impl CompileOptions {
    pub fn for_file(db: &RootDb, file: FileId) -> Self {
        let context = <dyn PreprocDb>::compilation_context_for_file(db, file);
        Self {
            predefines: context.predefines.to_vec(),
            include_dirs: context.include_dirs.iter().map(ToString::to_string).collect(),
            top_modules: context.top_modules.to_vec(),
        }
    }

    pub fn for_profile(db: &RootDb, profile: Option<CompilationProfileId>) -> Self {
        let context = <dyn PreprocDb>::compilation_context(db, profile);
        Self {
            predefines: context.predefines.to_vec(),
            include_dirs: context.include_dirs.iter().map(ToString::to_string).collect(),
            top_modules: context.top_modules.to_vec(),
        }
    }
}

/// Holds a [`SourceSession`] and the trees parsed on it. `compile` runs on
/// the calling thread.
pub struct Compiler {
    session: SourceSession,
    trees: FxHashMap<FileId, syntax::SyntaxTree>,
    hashes: FxHashMap<FileId, u64>,
    assigned: FxHashMap<String, String>,
}

/// C++ `shared_ptr` is not `Send` in cxx. Access is exclusive through
/// `Mutex<Compiler>` on the calling thread (P2 keystroke compile).
unsafe impl Send for Compiler {}

impl Compiler {
    pub fn new() -> Self {
        Self {
            session: SourceSession::new(),
            trees: FxHashMap::default(),
            hashes: FxHashMap::default(),
            assigned: FxHashMap::default(),
        }
    }

    #[cfg(test)]
    pub fn session(&self) -> &SourceSession {
        &self.session
    }

    /// Parse dirty/missing compilation-unit trees on this session and return
    /// a new [`Compilation`]. Include-only files are assigned as buffers, not
    /// parsed as roots.
    pub fn compile(
        &mut self,
        db: &RootDb,
        files: impl IntoIterator<Item = FileId>,
        options: &CompileOptions,
    ) -> Compilation {
        let files: Vec<FileId> = {
            let mut files: Vec<_> = files.into_iter().collect();
            files.sort_unstable_by_key(|file| file.index());
            files.dedup();
            files
        };

        let mut current_hashes = FxHashMap::default();
        for &file in &files {
            current_hashes.insert(file, hash_text(&db.file_text(file)));
            self.put_text(db, file);
            if db.file_kind(file).is_semantic_compilation_unit() {
                for buffer in compilation_plan::assigned_include_buffers_for_file(db, file) {
                    self.put_assigned(&buffer.path, &buffer.text);
                }
            }
        }

        let parse_options = SyntaxTreeOptions {
            predefines: options.predefines.clone(),
            include_paths: options.include_dirs.clone(),
            expand_includes: true,
            ..SyntaxTreeOptions::default()
        };

        for &file in &files {
            if !db.file_kind(file).is_semantic_compilation_unit() {
                continue;
            }
            if self.cu_is_fresh(db, file, &current_hashes) {
                continue;
            }
            let path = compilation_plan::source_buffer_path(db, file).to_string();
            let name =
                db.file_path(file).map(|path| path.to_string()).unwrap_or_else(|| path.clone());
            let tree = self.session.parse(&name, &path, &parse_options);
            self.trees.insert(file, tree);
        }
        self.hashes = current_hashes;

        let mut compilation = if options.top_modules.is_empty() {
            Compilation::on(&self.session)
        } else {
            Compilation::on_with_top_modules(&self.session, &options.top_modules)
        };
        for &file in &files {
            if !db.file_kind(file).is_semantic_compilation_unit() {
                continue;
            }
            if let Some(tree) = self.trees.get(&file) {
                compilation.add_syntax_tree(tree);
            }
        }
        compilation
    }

    fn cu_is_fresh(&self, db: &RootDb, file: FileId, current: &FxHashMap<FileId, u64>) -> bool {
        if !self.trees.contains_key(&file) {
            return false;
        }
        let mut inputs = vec![file];
        inputs.extend(<dyn PreprocDb>::static_include_closure(db, file).files().iter().copied());
        inputs.iter().all(|file| self.hashes.get(file) == current.get(file))
    }

    fn put_text(&mut self, db: &RootDb, file: FileId) {
        let path = compilation_plan::source_buffer_path(db, file).to_string();
        let text = db.file_text(file).to_string();
        self.put_assigned(&path, &text);
    }

    fn put_assigned(&mut self, path: &str, text: &str) {
        match self.assigned.get(path) {
            None => {
                self.session.assign(path, text);
                self.assigned.insert(path.to_owned(), text.to_owned());
            }
            Some(old) if old == text => {}
            Some(_) => {
                self.session.replace_buffer(path, text);
                self.assigned.insert(path.to_owned(), text.to_owned());
            }
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_root::{SourceRoot, SourceRootId},
    };
    use triomphe::Arc;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;

    const USER: FileId = FileId::from_raw(0);
    const PKG: FileId = FileId::from_raw(1);
    const LEAF: FileId = FileId::from_raw(2);
    const CHILD: FileId = FileId::from_raw(0);
    const TOP: FileId = FileId::from_raw(1);

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { r"C:\repo" } else { "/repo" };
        let sep = if cfg!(windows) { "\\" } else { "/" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}{sep}{path}")))
    }

    fn db_with_files(entries: &[(FileId, &str, &str)]) -> RootDb {
        let mut file_set = FileSet::default();
        let mut change = Change::new();
        for (file_id, path, text) in entries {
            file_set.insert(*file_id, VfsPath::from(abs_path(path)));
            change.add_changed_file(ChangedFile::create(*file_id, *text));
        }
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::default(),
            }],
        )));
        let mut db = RootDb::new(None);
        db.apply_change(change);
        db
    }

    fn include_into_package_db() -> RootDb {
        db_with_files(&[
            (
                USER,
                "user.sv",
                "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n",
            ),
            (PKG, "pkg.sv", "package pkg;\n  `include \"leaf.svh\"\nendpackage\n"),
            (LEAF, "leaf.svh", "class leaf;\n  string m_leaf_name;\nendclass\n"),
        ])
    }

    #[test]
    fn file_closure_includes_the_class_file_direct_names_do_not() {
        let db = include_into_package_db();
        let closure = file_closure(&db, USER);
        let direct = direct_names(&db, USER);
        assert!(closure.contains(USER), "{:?}", closure.files());
        assert!(closure.contains(PKG), "import pkg must locate pkg.sv: {:?}", closure.files());
        assert!(
            closure.contains(LEAF),
            "include-into-package must put leaf.svh in the closure: {:?}",
            closure.files()
        );
        assert!(direct.contains(USER));
        assert!(direct.contains(PKG), "direct names still see the import: {:?}", direct.files());
        assert!(
            !direct.contains(LEAF),
            "slang-server-shaped set must not walk pkg's include graph: {:?}",
            direct.files()
        );
    }

    #[test]
    fn compile_file_closure_answers_the_included_class_member() {
        let db = include_into_package_db();
        let mut compiler = Compiler::new();
        let closure = file_closure(&db, USER);
        let user = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n";
        let mut compilation = compiler.compile(
            &db,
            closure.files().iter().copied(),
            &CompileOptions::for_file(&db, USER),
        );
        let path = compilation_plan::source_buffer_path(&db, USER).to_string();
        let info = compilation
            .lookup_symbol(&path, user.find("m_leaf_name").expect("use"))
            .expect("closure compilation must see the class member");
        assert!(info.type_name.contains("string"), "{info:?}");
        assert_eq!(info.owner_class, "leaf", "{info:?}");
        assert_eq!(compiler.session().parse_count(), 2, "pkg.sv and user.sv, not leaf.svh as a CU");

        let edited = "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"y\";\nendmodule\n";
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(USER, edited));
        let mut db = db;
        db.apply_change(change);
        let closure = file_closure(&db, USER);
        let mut compilation = compiler.compile(
            &db,
            closure.files().iter().copied(),
            &CompileOptions::for_file(&db, USER),
        );
        let info = compilation
            .lookup_symbol(&path, edited.find("m_leaf_name").expect("use"))
            .expect("reused pkg tree must still bind the class");
        assert!(info.type_name.contains("string"), "{info:?}");
        assert_eq!(
            compiler.session().parse_count(),
            3,
            "body-only user.sv edit must not reparse pkg.sv"
        );
    }

    fn include_into_package_host() -> (crate::analysis_host::AnalysisHost, FileId, String) {
        let user =
            "module top;\n  pkg::leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n";
        let mut file_set = FileSet::default();
        let mut change = Change::new();
        for (file_id, path, text) in [
            (USER, "user.sv", user),
            (PKG, "pkg.sv", "package pkg;\n  `include \"leaf.svh\"\nendpackage\n"),
            (LEAF, "leaf.svh", "class leaf;\n  string m_leaf_name;\nendclass\n"),
        ] {
            file_set.insert(file_id, VfsPath::from(abs_path(path)));
            change.add_changed_file(ChangedFile::create(file_id, text));
        }
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::default(),
            }],
        )));
        let mut host = crate::analysis_host::AnalysisHost::without_elaboration();
        host.apply_change_without_prewarm(change);
        (host, USER, user.to_owned())
    }

    #[test]
    fn workerless_hover_type_on_include_into_package() {
        let (host, user, text) = include_into_package_host();
        let offset = utils::line_index::TextSize::from(text.find("m_leaf_name").unwrap() as u32);
        let hover = host
            .make_analysis()
            .hover(crate::FilePosition { file_id: user, offset })
            .unwrap()
            .expect("class member hover");
        let shown = hover.info.as_str();
        assert!(shown.contains("string"), "{shown}");
        assert!(!shown.contains("unknown"), "{shown}");
        assert!(!shown.contains("hir-ty"), "{shown}");
    }

    #[test]
    fn workerless_colon_colon_goto_on_include_into_package() {
        let (host, user, text) = include_into_package_host();
        let offset = utils::line_index::TextSize::from(text.find("leaf inst").unwrap() as u32);
        let nav = host
            .make_analysis()
            .goto_definition(crate::FilePosition { file_id: user, offset })
            .unwrap()
            .expect("pkg::leaf");
        assert!(
            nav.info.iter().any(|target| target.file_id == LEAF),
            "pkg::leaf must jump into the included class file, not catalog files[0]: {nav:?}"
        );
        assert_eq!(nav.info.len(), 1, "compilation binds; Ambiguous is not a jump: {nav:?}");
    }

    #[test]
    fn workerless_dot_members_on_include_into_package() {
        let (host, user, text) = include_into_package_host();
        let dot = text.find("inst.").expect("dot") + "inst.".len();
        let items = host
            .make_analysis()
            .completions_with_trigger(
                crate::FilePosition {
                    file_id: user,
                    offset: utils::line_index::TextSize::from(dot as u32),
                },
                None,
            )
            .unwrap();
        assert!(
            items.iter().any(|item| item.label.contains("m_leaf_name")),
            "inst. must complete the included class member: {items:?}"
        );
    }

    #[test]
    fn compile_a_two_file_profile_uses_the_same_function() {
        let db = db_with_files(&[
            (CHILD, "child.sv", "module child;\n  wire child_wire;\nendmodule\n"),
            (TOP, "top.sv", "module top;\n  child u0();\nendmodule\n"),
        ]);
        let mut compiler = Compiler::new();
        let files = [CHILD, TOP];
        let mut compilation = compiler.compile(
            &db,
            files,
            &CompileOptions::for_profile(&db, db.file_compilation_profile(TOP)),
        );
        let instances = compilation.list_instances();
        assert!(
            instances.iter().any(|inst| inst.path.contains("u0")),
            "profile compile of two files must elaborate the instance: {instances:?}"
        );
    }
}
