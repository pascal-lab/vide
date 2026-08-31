//! File-closure radius and a calling-thread [`compile`].
//!
//! This is the slang door for keystroke and profile compilations.
//! Callers pass a file set plus [`CompileOptions`]; they do not assemble parse
//! roots or assigned buffers. Closure and profile use the same
//! [`Compiler::compile`]. Hover, goto, `.` / `::`, and types wait on a
//! file-closure compile. Profile diagnostics pass the profile file set into
//! the same function.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use base_db::{
    diagnostics_config::{
        DiagnosticRuleSeverity, DiagnosticSelector, DiagnosticSource as SlangDiagnosticSource,
        DiagnosticsConfig,
    },
    project::CompilationProfileId,
    source_db::{SourceDb, SourceFileKind, SourceRootDb},
};
use design_graph::DesignGraphDb;
use preproc_expand::{
    compilation_plan,
    db::{CompilationDiagnostic, PreprocDb},
};
use rustc_hash::{FxHashMap, FxHashSet};
use slang_sys::compilation::{Compilation, HierInstance, SourceSession};
use syntax::{SyntaxTreeOptions, diagnostics::SyntaxDiagnostic};
use utils::{
    path_identity::PathIdentityIndex,
    paths::{AbsPathBuf, Utf8PathBuf},
};
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
    #[cfg(test)]
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
#[cfg(test)]
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
                for file in catalog.files_named_matching(&import.package, |kind| kind.is_package())
                {
                    absorb(db, file, walk_includes, &mut seen, &mut files, &mut pending);
                }
            }
            for package_ref in facts.package_refs.iter() {
                for file in
                    catalog.files_named_matching(&package_ref.name, |kind| kind.is_package())
                {
                    absorb(db, file, walk_includes, &mut seen, &mut files, &mut pending);
                }
            }
            for site in facts.instantiations.iter() {
                for file in catalog.files_for_role(&site.name, site.role) {
                    absorb(db, file, walk_includes, &mut seen, &mut files, &mut pending);
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

/// Product of one [`Compiler::compile`].
///
/// Callers query the [`Compilation`] value and the FileIds this compile
/// covered. They do not assemble parse roots, assigned buffers, or BufferIDs.
pub struct CompilationArtifact {
    compilation: Compilation,
    files: Vec<FileId>,
    path_files: PathIdentityIndex<FileId>,
    fingerprint: u64,
}

impl CompilationArtifact {
    pub fn files(&self) -> &[FileId] {
        &self.files
    }

    pub fn covers(&self, file: FileId) -> bool {
        self.files.contains(&file)
    }

    pub fn file_id_for_path(&self, path: &str) -> Option<FileId> {
        self.path_files.get(path)
    }

    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
}

impl Deref for CompilationArtifact {
    type Target = Compilation;

    fn deref(&self) -> &Compilation {
        &self.compilation
    }
}

impl DerefMut for CompilationArtifact {
    fn deref_mut(&mut self) -> &mut Compilation {
        &mut self.compilation
    }
}

/// Holds a [`SourceSession`] and the trees parsed on it. `compile` runs on
/// the calling thread.
///
/// `trees`, `hashes`, and `assigned` grow with paths this session has seen.
/// Replaced buffers stay because older [`Compilation`] values still name
/// those trees. The bound is the session lifetime on
/// [`crate::analysis_host::AnalysisHost`], not per compile.
pub struct Compiler {
    session: SourceSession,
    trees: FxHashMap<FileId, syntax::SyntaxTree>,
    hashes: FxHashMap<FileId, u64>,
    assigned: FxHashMap<String, String>,
    parse_fingerprint: u64,
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
            parse_fingerprint: 0,
        }
    }

    #[cfg(test)]
    pub fn session(&self) -> &SourceSession {
        &self.session
    }

    /// Parse dirty/missing compilation-unit trees on this session and return
    /// a [`CompilationArtifact`]. Files in another root's include graph are
    /// assigned as buffers, not parsed as roots — including `.sv` fragments.
    pub fn compile(
        &mut self,
        db: &RootDb,
        files: impl IntoIterator<Item = FileId>,
        options: &CompileOptions,
    ) -> CompilationArtifact {
        self.compile_inner(db, files, options, &[])
    }

    fn compile_inner(
        &mut self,
        db: &RootDb,
        files: impl IntoIterator<Item = FileId>,
        options: &CompileOptions,
        extra: &[compilation_plan::AssignedIncludeBuffer],
    ) -> CompilationArtifact {
        let files: Vec<FileId> = {
            let mut files: Vec<_> = files.into_iter().collect();
            files.sort_unstable_by_key(|file| file.index());
            files.dedup();
            files
        };

        let mut current_hashes = FxHashMap::default();
        for &file in &files {
            current_hashes.insert(file, hash_file(db, file));
            self.put_text(db, file);
            if db.file_kind(file).is_semantic_compilation_unit() {
                for &included in <dyn PreprocDb>::static_include_closure(db, file).files() {
                    current_hashes.insert(included, hash_file(db, included));
                }
                for buffer in compilation_plan::assigned_include_buffers_for_file(db, file) {
                    current_hashes.insert(buffer.file_id, hash_text(&buffer.text));
                    self.put_assigned(&buffer.path, &buffer.text);
                }
            }
        }
        for buffer in extra {
            current_hashes.insert(buffer.file_id, hash_text(&buffer.text));
            self.put_assigned(&buffer.path, &buffer.text);
        }

        let parse_options = SyntaxTreeOptions {
            predefines: options.predefines.clone(),
            include_paths: options.include_dirs.clone(),
            expand_includes: true,
            ..SyntaxTreeOptions::default()
        };
        let parse_fingerprint = parse_fingerprint(options, &parse_options, extra);

        let roots = parse_roots(db, &files);
        for &file in &roots {
            if self.cu_is_fresh(db, file, &current_hashes, &files, extra, parse_fingerprint) {
                continue;
            }
            let path = compilation_plan::source_buffer_path(db, file).to_string();
            let name =
                db.file_path(file).map(|path| path.to_string()).unwrap_or_else(|| path.clone());
            let tree = match db.file_kind(file) {
                SourceFileKind::LibraryMap => {
                    self.session.parse_library_map(&name, &path, &parse_options)
                }
                _ => self.session.parse(&name, &path, &parse_options),
            };
            self.trees.insert(file, tree);
        }
        self.hashes = current_hashes;
        self.parse_fingerprint = parse_fingerprint;

        let mut compilation = if options.top_modules.is_empty() {
            Compilation::on(&self.session)
        } else {
            Compilation::on_with_top_modules(&self.session, &options.top_modules)
        };
        for &file in &roots {
            if let Some(tree) = self.trees.get(&file) {
                compilation.add_syntax_tree(tree);
            }
        }
        let (files, path_files) = covered_buffers(db, &files, extra);
        CompilationArtifact { compilation, files, path_files, fingerprint: parse_fingerprint }
    }

    /// Hierarchical instances on a compilation of `files`.
    pub fn instances(
        &mut self,
        db: &RootDb,
        files: impl IntoIterator<Item = FileId>,
        options: &CompileOptions,
    ) -> Vec<HierInstance> {
        self.compile(db, files, options).list_instances()
    }

    /// Parse + semantic diagnostics on a compilation of `files`.
    ///
    /// The request does not spawn a process. Profile callers pass the profile
    /// file set; keystroke callers pass a file closure.
    pub fn diagnostics(
        &mut self,
        db: &RootDb,
        files: impl IntoIterator<Item = FileId>,
        options: &CompileOptions,
        config: &DiagnosticsConfig,
    ) -> Vec<CompilationDiagnostic> {
        let files: Vec<FileId> = {
            let mut files: Vec<_> = files.into_iter().collect();
            files.sort_unstable_by_key(|file| file.index());
            files.dedup();
            files
        };
        let extra = files
            .iter()
            .find_map(|&file| db.file_compilation_profile(file))
            .map(|profile| {
                let plan = <dyn PreprocDb>::compilation_plan_for_profile(db, Some(profile));
                compilation_plan::compilation_source_buffers_for_plan(db, &plan)
            })
            .unwrap_or_default();
        let compilation = self.compile_inner(db, files.iter().copied(), options, &extra);
        let buffer_file_ids = self.buffer_file_ids(db, &files);
        let warning_options = warning_options(config);
        let mut diagnostics = Vec::new();
        if config.enabled && config.parse.enabled {
            collect_diagnostics(
                config,
                SlangDiagnosticSource::Parse,
                compilation.parse_diagnostics_with_options(&warning_options),
                &buffer_file_ids,
                &mut diagnostics,
            );
        }
        if config.enabled && config.semantic.enabled {
            collect_diagnostics(
                config,
                SlangDiagnosticSource::Semantic,
                compilation.semantic_diagnostics_with_options(&warning_options),
                &buffer_file_ids,
                &mut diagnostics,
            );
        }
        diagnostics
    }

    fn buffer_file_ids(&self, db: &RootDb, files: &[FileId]) -> FxHashMap<u32, FileId> {
        let mut path_files = FxHashMap::<String, FileId>::default();
        for &file in files {
            path_files.insert(compilation_plan::source_buffer_path(db, file).to_string(), file);
            if db.file_kind(file).is_semantic_compilation_unit() {
                for buffer in compilation_plan::assigned_include_buffers_for_file(db, file) {
                    path_files.insert(buffer.path, buffer.file_id);
                }
            }
        }
        if let Some(profile) = files.iter().find_map(|&file| db.file_compilation_profile(file)) {
            let plan = <dyn PreprocDb>::compilation_plan_for_profile(db, Some(profile));
            for buffer in compilation_plan::compilation_source_buffers_for_plan(db, &plan) {
                path_files.insert(buffer.path, buffer.file_id);
            }
        }
        let mut map = FxHashMap::default();
        for (&file, tree) in &self.trees {
            let ids = tree.buffer_ids();
            map.insert(ids.root_buffer_id, file);
            for source in ids.source_buffers {
                if let Some(&file_id) = path_files.get(&source.path) {
                    map.insert(source.buffer_id, file_id);
                }
            }
        }
        map
    }

    fn cu_is_fresh(
        &self,
        db: &RootDb,
        file: FileId,
        current: &FxHashMap<FileId, u64>,
        files: &[FileId],
        extra: &[compilation_plan::AssignedIncludeBuffer],
        parse_fingerprint: u64,
    ) -> bool {
        if self.parse_fingerprint != parse_fingerprint {
            return false;
        }
        if !self.trees.contains_key(&file) {
            return false;
        }
        let mut inputs = vec![file];
        inputs.extend(<dyn PreprocDb>::static_include_closure(db, file).files().iter().copied());
        inputs.extend(
            files
                .iter()
                .copied()
                .filter(|&file| !db.file_kind(file).is_semantic_compilation_unit()),
        );
        inputs.extend(
            extra
                .iter()
                .map(|buffer| buffer.file_id)
                .filter(|&file| !db.file_kind(file).is_semantic_compilation_unit()),
        );
        inputs.iter().all(|file| self.hashes.get(file) == current.get(file))
    }

    fn put_text(&mut self, db: &RootDb, file: FileId) -> bool {
        let path = compilation_plan::source_buffer_path(db, file).to_string();
        let text = db.file_text(file).to_string();
        self.put_assigned(&path, &text)
    }

    fn put_assigned(&mut self, path: &str, text: &str) -> bool {
        match self.assigned.get(path) {
            None => {
                self.session.assign(path, text);
                self.assigned.insert(path.to_owned(), text.to_owned());
                true
            }
            Some(old) if old == text => false,
            Some(_) => {
                self.session.replace_buffer(path, text);
                self.assigned.insert(path.to_owned(), text.to_owned());
                true
            }
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

fn covered_buffers(
    db: &RootDb,
    files: &[FileId],
    extra: &[compilation_plan::AssignedIncludeBuffer],
) -> (Vec<FileId>, PathIdentityIndex<FileId>) {
    let mut seen = FxHashSet::default();
    let mut covered = Vec::new();
    let mut path_files = PathIdentityIndex::default();
    for &file in files {
        absorb_covered(db, file, &mut seen, &mut covered, &mut path_files);
        if db.file_kind(file).is_semantic_compilation_unit() {
            for &included in <dyn PreprocDb>::static_include_closure(db, file).files() {
                absorb_covered(db, included, &mut seen, &mut covered, &mut path_files);
            }
            for buffer in compilation_plan::assigned_include_buffers_for_file(db, file) {
                absorb_assigned(&buffer, &mut seen, &mut covered, &mut path_files);
            }
        }
    }
    for buffer in extra {
        absorb_assigned(buffer, &mut seen, &mut covered, &mut path_files);
    }
    covered.sort_unstable_by_key(|file| file.index());
    (covered, path_files)
}

fn absorb_covered(
    db: &RootDb,
    file: FileId,
    seen: &mut FxHashSet<FileId>,
    covered: &mut Vec<FileId>,
    path_files: &mut PathIdentityIndex<FileId>,
) {
    if seen.insert(file) {
        covered.push(file);
    }
    let path = compilation_plan::source_buffer_path(db, file);
    path_files.insert_path(path.as_path(), file);
}

fn absorb_assigned(
    buffer: &compilation_plan::AssignedIncludeBuffer,
    seen: &mut FxHashSet<FileId>,
    covered: &mut Vec<FileId>,
    path_files: &mut PathIdentityIndex<FileId>,
) {
    if seen.insert(buffer.file_id) {
        covered.push(buffer.file_id);
    }
    let path = AbsPathBuf::try_from(Utf8PathBuf::from(buffer.path.as_str()))
        .unwrap_or_else(|_| panic!("compile assigned a non-absolute buffer path: {}", buffer.path));
    path_files.insert_path(path.as_path(), buffer.file_id);
}

/// CUs in `files` that are not in another file's include graph.
///
/// Same rule as [`compilation_plan::CompilationPlan::include_only`]: an
/// included `.sv` is a buffer. Files unreachable from the remaining roots
/// (include cycles) stay roots so they are not dropped.
fn parse_roots(db: &RootDb, files: &[FileId]) -> Vec<FileId> {
    let cus: Vec<FileId> = files
        .iter()
        .copied()
        .filter(|&file| db.file_kind(file).is_semantic_compilation_unit())
        .collect();
    let mut included = FxHashSet::default();
    for &file in &cus {
        for &fragment in <dyn PreprocDb>::static_include_closure(db, file).files() {
            if fragment != file {
                included.insert(fragment);
            }
        }
    }
    let mut roots: Vec<FileId> =
        cus.iter().copied().filter(|file| !included.contains(file)).collect();
    if roots.is_empty() {
        return cus;
    }
    let mut reachable = FxHashSet::default();
    let mut pending = roots.clone();
    while let Some(file) = pending.pop() {
        for &fragment in <dyn PreprocDb>::static_include_closure(db, file).files() {
            if reachable.insert(fragment) {
                pending.push(fragment);
            }
        }
    }
    for file in cus {
        if !roots.contains(&file) && !reachable.contains(&file) {
            roots.push(file);
        }
    }
    roots.sort_unstable_by_key(|file| file.index());
    roots
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn hash_file(db: &RootDb, file: FileId) -> u64 {
    let mut hasher = DefaultHasher::new();
    db.file_text(file).hash(&mut hasher);
    db.file_kind(file).hash(&mut hasher);
    hasher.finish()
}

fn parse_fingerprint(
    options: &CompileOptions,
    parse_options: &SyntaxTreeOptions,
    extra: &[compilation_plan::AssignedIncludeBuffer],
) -> u64 {
    let mut hasher = DefaultHasher::new();
    options.predefines.hash(&mut hasher);
    options.include_dirs.hash(&mut hasher);
    parse_options.expand_includes.hash(&mut hasher);
    parse_options.collect_expected_syntax.hash(&mut hasher);
    parse_options.expected_syntax_offset.hash(&mut hasher);
    for buffer in extra {
        buffer.path.hash(&mut hasher);
        buffer.text.hash(&mut hasher);
    }
    hasher.finish()
}

fn warning_options(config: &DiagnosticsConfig) -> Vec<String> {
    match &config.slang.warnings {
        Some(options) if options.is_empty() => vec!["none".to_owned()],
        Some(options) => options.clone(),
        None => Vec::new(),
    }
}

fn collect_diagnostics(
    config: &DiagnosticsConfig,
    source: SlangDiagnosticSource,
    raw: Vec<SyntaxDiagnostic>,
    buffer_file_ids: &FxHashMap<u32, FileId>,
    diagnostics: &mut Vec<CompilationDiagnostic>,
) {
    diagnostics.extend(raw.into_iter().filter_map(|diagnostic| {
        let file_id =
            diagnostic.buffer_id.and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied())?;
        let diagnostic = apply_rules(config, source, diagnostic)?;
        Some(CompilationDiagnostic { file_id, source, diagnostic })
    }));
}

fn apply_rules(
    config: &DiagnosticsConfig,
    source: SlangDiagnosticSource,
    mut diagnostic: SyntaxDiagnostic,
) -> Option<SyntaxDiagnostic> {
    use syntax::diagnostics::DiagnosticSeverity;
    for rule in &config.slang.rules {
        let matches = match &rule.selector {
            DiagnosticSelector::Code { subsystem, code } => {
                diagnostic.subsystem == *subsystem && diagnostic.code == *code
            }
            DiagnosticSelector::Option(option) => diagnostic.option_name.as_deref() == Some(option),
            DiagnosticSelector::Group(group) => {
                diagnostic.groups.iter().any(|candidate| candidate == group)
            }
            DiagnosticSelector::Source(rule_source) => source == *rule_source,
        };
        if !matches {
            continue;
        }
        diagnostic.severity = match rule.severity {
            DiagnosticRuleSeverity::Ignore => return None,
            DiagnosticRuleSeverity::Info => DiagnosticSeverity::Note,
            DiagnosticRuleSeverity::Warning => DiagnosticSeverity::Warning,
            DiagnosticRuleSeverity::Error => DiagnosticSeverity::Error,
            DiagnosticRuleSeverity::Fatal => DiagnosticSeverity::Fatal,
        };
    }
    (diagnostic.severity != DiagnosticSeverity::Ignored).then_some(diagnostic)
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

    /// Same shape as UVM/riscv-dv: the included class file is `.sv`, so
    /// `file_kind` calls it a compilation unit. It is still a fragment.
    fn include_into_package_sv_db() -> RootDb {
        db_with_files(&[
            (
                USER,
                "user.sv",
                "module top;\n  import pkg::*;\n  leaf inst;\n  initial inst.m_leaf_name = \"x\";\nendmodule\n",
            ),
            (PKG, "pkg.sv", "package pkg;\n  `include \"leaf.sv\"\nendpackage\n"),
            (LEAF, "leaf.sv", "class leaf;\n  string m_leaf_name;\nendclass\n"),
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

    #[test]
    fn compile_does_not_parse_an_included_sv_as_a_cu_root() {
        let db = include_into_package_sv_db();
        let mut compiler = Compiler::new();
        let closure = file_closure(&db, USER);
        assert!(closure.contains(LEAF), "included .sv must still be in the closure");
        let _ = compiler.compile(
            &db,
            closure.files().iter().copied(),
            &CompileOptions::for_file(&db, USER),
        );
        assert_eq!(
            compiler.session().parse_count(),
            2,
            "pkg.sv and user.sv; leaf.sv is a buffer even though file_kind is SystemVerilog"
        );

        let mut profile = Compiler::new();
        let _ = profile.compile(
            &db,
            [USER, PKG, LEAF],
            &CompileOptions::for_profile(&db, db.file_compilation_profile(USER)),
        );
        assert_eq!(
            profile.session().parse_count(),
            2,
            "profile of the same three files must not parse leaf.sv as a second root"
        );

        let before = compiler.session().parse_count();
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(
            LEAF,
            "class leaf;\n  string m_leaf_name;\n  string extra;\nendclass\n",
        ));
        let mut db = db;
        db.apply_change(change);
        let closure = file_closure(&db, USER);
        let _ = compiler.compile(
            &db,
            closure.files().iter().copied(),
            &CompileOptions::for_file(&db, USER),
        );
        assert_eq!(
            compiler.session().parse_count().saturating_sub(before),
            1,
            "leaf.sv edit must reparse pkg.sv (the root), not leaf.sv as a CU"
        );
    }

    #[test]
    fn compile_file_closure_answers_the_included_sv_class_member() {
        let db = include_into_package_sv_db();
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
            .expect("included .sv class member must bind; a second CU root duplicates the class");
        assert!(info.type_name.contains("string"), "{info:?}");
        assert_eq!(info.owner_class, "leaf", "{info:?}");
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
    fn workerless_dot_member_goto_on_include_into_package() {
        let (host, user, text) = include_into_package_host();
        let offset = utils::line_index::TextSize::from(text.find("m_leaf_name").unwrap() as u32);
        let nav = host
            .make_analysis()
            .goto_definition(crate::FilePosition { file_id: user, offset })
            .unwrap()
            .expect("inst.m_leaf_name");
        assert!(
            nav.info.iter().any(|target| target.file_id == LEAF),
            "dot member must jump into the included class file: {nav:?}"
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
    fn compile_artifact_maps_only_this_compilation_buffers() {
        let other = FileId::from_raw(3);
        let db = db_with_files(&[
            (USER, "user.sv", "module top;\n  import pkg::*;\n  leaf inst;\nendmodule\n"),
            (PKG, "pkg.sv", "package pkg;\n  `include \"leaf.svh\"\nendpackage\n"),
            (LEAF, "leaf.svh", "class leaf;\nendclass\n"),
            (other, "other.sv", "module other;\nendmodule\n"),
        ]);
        let mut compiler = Compiler::new();
        let artifact = compiler.compile(&db, [USER, PKG], &CompileOptions::for_file(&db, USER));
        let leaf_path = compilation_plan::source_buffer_path(&db, LEAF).to_string();
        let other_path = compilation_plan::source_buffer_path(&db, other).to_string();
        let _fingerprint = artifact.fingerprint();
        assert!(
            artifact.covers(LEAF),
            "assigned includes must be on the artifact; callers do not assemble buffers: {:?}",
            artifact.files()
        );
        assert!(
            !artifact.covers(other),
            "unrelated workspace files are not this compile: {:?}",
            artifact.files()
        );
        assert_eq!(artifact.file_id_for_path(&leaf_path), Some(LEAF));
        assert_eq!(
            artifact.file_id_for_path(&other_path),
            None,
            "workspace path index must not leak files this compilation did not cover"
        );
    }

    #[test]
    fn compile_file_closure_answers_the_instantiation_type() {
        let db = db_with_files(&[
            (CHILD, "child.sv", "module child;\nendmodule\n"),
            (TOP, "top.sv", "module top;\n  child u0();\nendmodule\n"),
        ]);
        let mut compiler = Compiler::new();
        let closure = file_closure(&db, TOP);
        assert!(closure.contains(CHILD), "{:?}", closure.files());
        let user = "module top;\n  child u0();\nendmodule\n";
        let mut compilation = compiler.compile(
            &db,
            closure.files().iter().copied(),
            &CompileOptions::for_file(&db, TOP),
        );
        let path = compilation_plan::source_buffer_path(&db, TOP).to_string();
        let info = compilation
            .lookup_symbol(&path, user.find("child u0").expect("type"))
            .expect("closure compilation must bind the instantiation type");
        assert_eq!(info.name, "child", "{info:?}");
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

    #[test]
    fn compile_profile_diagnostics_are_in_process() {
        let db = db_with_files(&[
            (CHILD, "child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            (TOP, "top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ]);
        let mut compiler = Compiler::new();
        let diagnostics = compiler.diagnostics(
            &db,
            [CHILD, TOP],
            &CompileOptions::for_profile(&db, db.file_compilation_profile(TOP)),
            db.diagnostics_config().as_ref(),
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.file_id == TOP
                    && diagnostic.diagnostic.message.contains("input port 'b' has no connection")
            }),
            "profile compilation must emit the missing-port diagnostic without a process: {diagnostics:?}"
        );
    }

    #[test]
    fn included_buffer_diagnostics_keep_their_file_identity() {
        use base_db::project::PreprocessConfig;

        let header = FileId::from_raw(0);
        let user = FileId::from_raw(1);
        let root = abs_path("");
        let mut file_set = FileSet::default();
        let mut change = Change::new();
        file_set.insert(header, VfsPath::from(abs_path("defs.svh")));
        file_set.insert(user, VfsPath::from(abs_path("user.sv")));
        change.add_changed_file(ChangedFile::create(header, "logic value;\nlogic value;\n"));
        change.add_changed_file(ChangedFile::create(
            user,
            "`include \"defs.svh\"\nmodule top; endmodule\n",
        ));
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        let mut db = RootDb::new(None);
        db.apply_change(change);
        let mut compiler = Compiler::new();
        let diagnostics = compiler.diagnostics(
            &db,
            [header, user],
            &CompileOptions::for_profile(&db, db.file_compilation_profile(user)),
            db.diagnostics_config().as_ref(),
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.file_id == header),
            "include diagnostics must keep the header file id: {diagnostics:?}"
        );
    }

    #[test]
    fn include_overlay_edit_reparses_dependents() {
        use base_db::project::PreprocessConfig;

        let header = FileId::from_raw(0);
        let user = FileId::from_raw(1);
        let root = abs_path("");
        let mut file_set = FileSet::default();
        let mut change = Change::new();
        file_set.insert(header, VfsPath::from(abs_path("defs.svh")));
        file_set.insert(user, VfsPath::from(abs_path("user.sv")));
        change.add_changed_file(ChangedFile::create(header, "`define ENABLE 1\n"));
        change.add_changed_file(ChangedFile::create(
            user,
            "`include \"defs.svh\"\nmodule top;\n  logic enable = `ENABLE;\nendmodule\n",
        ));
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        let mut db = RootDb::new(None);
        db.apply_change(change);
        let options = CompileOptions::for_profile(&db, db.file_compilation_profile(user));
        let first = Compiler::new().diagnostics(
            &db,
            [header, user],
            &options,
            db.diagnostics_config().as_ref(),
        );
        assert!(
            first.iter().all(|diagnostic| !diagnostic.diagnostic.message.contains("ENABLE")),
            "defined macro should compile cleanly: {first:?}"
        );

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(header, ""));
        db.apply_change(change);
        let after = Compiler::new().diagnostics(
            &db,
            [header, user],
            &options,
            db.diagnostics_config().as_ref(),
        );
        assert!(!after.is_empty(), "empty include overlay must affect dependents: {after:?}");
    }

    #[test]
    fn source_rule_can_drop_parse_diagnostics() {
        use base_db::diagnostics_config::{
            DiagnosticRule, DiagnosticRuleSeverity, DiagnosticSelector, DiagnosticSource,
        };

        let db = db_with_files(&[(TOP, "top.sv", "module top(;\nendmodule\n")]);
        let mut config = db.diagnostics_config().as_ref().clone();
        config.semantic.enabled = false;
        config.slang.rules.push(DiagnosticRule {
            selector: DiagnosticSelector::Source(DiagnosticSource::Parse),
            severity: DiagnosticRuleSeverity::Ignore,
            force: false,
        });
        let mut compiler = Compiler::new();
        let diagnostics =
            compiler.diagnostics(&db, [TOP], &CompileOptions::for_file(&db, TOP), &config);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    fn db_without_profile(entries: &[(FileId, &str, &str)]) -> RootDb {
        let mut file_set = FileSet::default();
        let mut change = Change::new();
        for (file_id, path, text) in entries {
            file_set.insert(*file_id, VfsPath::from(abs_path(path)));
            change.add_changed_file(ChangedFile::create(*file_id, *text));
        }
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(Vec::new(), Vec::new())));
        let mut db = RootDb::new(None);
        db.apply_change(change);
        db
    }

    #[test]
    fn file_closure_of_an_orphan_is_this_file_and_includes() {
        let header = FileId::from_raw(2);
        let db = db_without_profile(&[
            (USER, "user.sv", "`include \"defs.svh\"\nmodule top;\n  import pkg::*;\nendmodule\n"),
            (PKG, "pkg.sv", "package pkg;\nendpackage\n"),
            (header, "defs.svh", "`define ENABLE 1\n"),
        ]);
        let closure = file_closure(&db, USER);
        assert!(closure.contains(USER), "{:?}", closure.files());
        assert!(
            closure.contains(header),
            "orphan files still walk the include graph: {:?}",
            closure.files()
        );
        assert!(
            !closure.contains(PKG),
            "unconfigured files must not pull named packages from the catalog: {:?}",
            closure.files()
        );
    }

    #[test]
    fn file_closure_keeps_every_duplicate_module_candidate() {
        let child_a = FileId::from_raw(2);
        let child_b = FileId::from_raw(3);
        let db = db_with_files(&[
            (TOP, "top.sv", "module top;\n  child u();\nendmodule\n"),
            (child_a, "a/child.sv", "module child;\nendmodule\n"),
            (child_b, "b/child.sv", "module child;\nendmodule\n"),
        ]);
        let closure = file_closure(&db, TOP);
        assert!(closure.contains(TOP), "{:?}", closure.files());
        assert!(
            closure.contains(child_a) && closure.contains(child_b),
            "duplicate module names contribute every catalog candidate: {:?}",
            closure.files()
        );
    }

    #[test]
    fn include_cycle_keeps_files_that_are_not_covered_by_remaining_roots() {
        let a = FileId::from_raw(0);
        let b = FileId::from_raw(1);
        let db = db_with_files(&[
            (a, "a.sv", "`include \"b.sv\"\nmodule a;\nendmodule\n"),
            (b, "b.sv", "`include \"a.sv\"\nmodule b;\nendmodule\n"),
        ]);
        let mut compiler = Compiler::new();
        let _ = compiler.compile(
            &db,
            [a, b],
            &CompileOptions::for_profile(&db, db.file_compilation_profile(a)),
        );
        assert_eq!(
            compiler.session().parse_count(),
            2,
            "an include cycle must not drop a CU that no remaining root covers"
        );
    }

    #[test]
    fn compile_reparses_when_predefines_change() {
        let src = "module top;\n  logic enable = `ENABLE;\nendmodule\n";
        let db = db_with_files(&[(TOP, "top.sv", src)]);
        let mut compiler = Compiler::new();
        let mut options = CompileOptions::for_file(&db, TOP);
        options.predefines = vec!["ENABLE=1".to_owned()];
        let _ = compiler.compile(&db, [TOP], &options);
        assert_eq!(compiler.session().parse_count(), 1);
        options.predefines = vec!["ENABLE=2".to_owned()];
        let _ = compiler.compile(&db, [TOP], &options);
        assert_eq!(
            compiler.session().parse_count(),
            2,
            "changing predefines must not reuse the previous tree"
        );
    }

    #[test]
    fn compile_reparses_when_include_dirs_change() {
        let src = "`include \"defs.svh\"\nmodule top;\nendmodule\n";
        let db = db_with_files(&[(TOP, "top.sv", src)]);
        let mut compiler = Compiler::new();
        let mut options = CompileOptions::for_file(&db, TOP);
        options.include_dirs = vec!["/repo".to_owned()];
        let _ = compiler.compile(&db, [TOP], &options);
        let first = compiler.session().parse_count();
        options.include_dirs = vec!["/other".to_owned()];
        let _ = compiler.compile(&db, [TOP], &options);
        assert_eq!(
            compiler.session().parse_count().saturating_sub(first),
            1,
            "changing include dirs must not reuse the previous tree"
        );
    }
}
