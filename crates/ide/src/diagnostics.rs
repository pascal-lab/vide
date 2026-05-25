use base_db::{
    diagnostics_config::DiagnosticSource as SlangDiagnosticSource,
    project::CompilationProfileId,
    source_db::{SourceDb, SourceRootDb},
    source_root::{SourceRootDiagnosticScope, SourceRootRole},
};
use hir::{db::HirDb, hir_def::module::ModuleId, source_map::IsSrc};
use ide_db::root_db::RootDb;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::{
    get::Get,
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

use crate::module_resolution::{ModuleResolution, ModuleResolutionAmbiguity, resolve_module_name};

const AMBIGUOUS_MODULE_INSTANTIATION: VizslaDiagnosticDescriptor =
    VizslaDiagnosticDescriptor { code: 1, subsystem: 0, name: "ambiguous-module-instantiation" };
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_STRICT: &str = "diagnostic.ambiguous_module.strict";
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_BEST_EFFORT: &str = "diagnostic.ambiguous_module.best_effort";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    SlangParse,
    SlangSemantic,
    Vizsla,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub code: u16,
    pub subsystem: u16,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub source: DiagnosticSource,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub message_key: Option<&'static str>,
    pub message_args: Vec<(&'static str, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VizslaDiagnosticDescriptor {
    code: u16,
    subsystem: u16,
    name: &'static str,
}

impl VizslaDiagnosticDescriptor {
    fn diagnostic(
        self,
        file_id: FileId,
        range: TextRange,
        severity: DiagnosticSeverity,
        message: String,
        message_key: &'static str,
        message_args: Vec<(&'static str, String)>,
    ) -> Diagnostic {
        Diagnostic {
            file_id,
            code: self.code,
            subsystem: self.subsystem,
            name: self.name.to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: DiagnosticSource::Vizsla,
            range,
            severity,
            message,
            message_key: Some(message_key),
            message_args,
        }
    }
}

pub(crate) fn parse_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    db.parse_diagnostics(file_id)
        .iter()
        .map(|diag| slang_diagnostic(file_id, SlangDiagnosticSource::Parse, diag))
        .collect()
}

fn compilation_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    db.file_compilation_diagnostics(file_id)
        .iter()
        .map(|diag| slang_diagnostic(diag.file_id, diag.source, &diag.diagnostic))
        .collect()
}

pub(crate) fn compilation_profile_diagnostics(
    db: &RootDb,
    profile_id: CompilationProfileId,
) -> Vec<Diagnostic> {
    db.compilation_profile_diagnostics(profile_id)
        .iter()
        .map(|diag| slang_diagnostic(diag.file_id, diag.source, &diag.diagnostic))
        .collect()
}

pub(crate) fn compilation_profile_syntax_diagnostics(
    db: &RootDb,
    profile_id: CompilationProfileId,
) -> Vec<Diagnostic> {
    let plan = db.compilation_plan_for_profile(Some(profile_id));
    let mut file_ids = plan.roots.clone();
    file_ids.extend(plan.include_only.iter().copied());
    file_ids.sort_unstable_by_key(|file_id| file_id.0);
    file_ids.dedup();

    file_ids.into_iter().flat_map(|file_id| syntax_diagnostics(db, file_id)).collect()
}

fn syntax_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = parse_diagnostics(db, file_id);
    diagnostics.extend(vizsla_diagnostics(db, file_id));
    diagnostics
}

fn slang_diagnostic(
    file_id: FileId,
    source: SlangDiagnosticSource,
    diag: &SyntaxDiagnostic,
) -> Diagnostic {
    Diagnostic {
        file_id,
        code: diag.code,
        subsystem: diag.subsystem,
        name: diag.name.clone(),
        option_name: diag.option_name.clone(),
        groups: diag.groups.clone(),
        source: match source {
            SlangDiagnosticSource::Parse => DiagnosticSource::SlangParse,
            SlangDiagnosticSource::Semantic => DiagnosticSource::SlangSemantic,
        },
        range: to_text_range(diag),
        severity: diag.severity,
        message: diag.message.clone(),
        message_key: None,
        message_args: Vec::new(),
    }
}

pub(crate) fn diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let source_root_id = db.source_root_id(file_id);
    // Ignored roots in a profiled workspace are explicitly outside the
    // diagnostic model. Profile-less workspaces still use open-file syntax
    // diagnostics for ad hoc files.
    if db.source_root(source_root_id).role().diagnostic_scope()
        == SourceRootDiagnosticScope::Disabled
        && db.project_config().has_compilation_profiles()
    {
        return Vec::new();
    }

    let mut diagnostics = if slang_semantic_diagnostics_active(db, file_id) {
        Vec::new()
    } else {
        syntax_diagnostics(db, file_id)
    };

    diagnostics.extend(
        compilation_diagnostics(db, file_id).into_iter().filter(|diag| diag.file_id == file_id),
    );

    diagnostics
}

pub(crate) fn source_root_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let source_root_id = db.source_root_id(file_id);
    let source_root = db.source_root(source_root_id);
    match source_root.role().diagnostic_scope() {
        SourceRootDiagnosticScope::Disabled => return Vec::new(),
        SourceRootDiagnosticScope::OpenFile => {
            return syntax_diagnostics(db, file_id);
        }
        SourceRootDiagnosticScope::Workspace => {}
    }

    let mut diagnostics = Vec::new();

    if slang_semantic_diagnostics_active(db, file_id) {
        diagnostics.extend(compilation_diagnostics(db, file_id));
    } else {
        for file_id in source_root.iter() {
            diagnostics.extend(syntax_diagnostics(db, file_id));
        }

        diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().map(
            |(diag_file_id, diag)| {
                slang_diagnostic(*diag_file_id, SlangDiagnosticSource::Semantic, diag)
            },
        ));
    }

    diagnostics
}

pub(crate) fn source_root_file_ids(db: &RootDb, file_id: FileId) -> Vec<FileId> {
    let source_root_id = db.source_root_id(file_id);
    let source_root = db.source_root(source_root_id);
    match source_root.role().diagnostic_scope() {
        SourceRootDiagnosticScope::Workspace => source_root.iter().collect(),
        SourceRootDiagnosticScope::OpenFile | SourceRootDiagnosticScope::Disabled => vec![file_id],
    }
}

pub(crate) fn source_root_role(db: &RootDb, file_id: FileId) -> SourceRootRole {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).role()
}

fn vizsla_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if slang_semantic_diagnostics_active(db, file_id) {
        return Vec::new();
    }

    module_instantiation_resolution_diagnostics(db, file_id)
}

fn slang_semantic_diagnostics_active(db: &RootDb, file_id: FileId) -> bool {
    let config = db.diagnostics_config();
    config.enabled
        && config.semantic.enabled
        && !db.file_is_project_ignored(file_id)
        && db.project_config().profile_for_root(db.source_root_id(file_id)).is_some()
}

fn module_instantiation_resolution_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let hir_file_id = file_id.into();
    let hir_file = db.hir_file(hir_file_id);
    let mut diagnostics = Vec::new();

    for (local_module_id, _) in hir_file.modules.iter() {
        let module_id = ModuleId::new(hir_file_id, local_module_id);
        let (module, src_map) = db.module_with_source_map(module_id);
        for (instantiation_id, instantiation) in module.instantiations.iter() {
            let Some(module_name) = instantiation.module_name.as_ref() else {
                continue;
            };
            let range = src_map
                .get(instantiation_id)
                .map(|src| src.range())
                .unwrap_or_else(|| TextRange::empty(TextSize::new(0)));

            match resolve_module_name(db, file_id, module_name) {
                ModuleResolution::Ambiguous { candidates, kind } => {
                    let (severity, message, message_key, message_args) =
                        ambiguous_module_instantiation_diagnostic(
                            module_name,
                            candidates.len(),
                            kind,
                        );
                    diagnostics.push(AMBIGUOUS_MODULE_INSTANTIATION.diagnostic(
                        file_id,
                        range,
                        severity,
                        message,
                        message_key,
                        message_args,
                    ));
                }
                ModuleResolution::Unique(_)
                | ModuleResolution::BestEffortProximity { .. }
                | ModuleResolution::Unresolved => {}
            }
        }
    }

    diagnostics
}

fn ambiguous_module_instantiation_diagnostic(
    module_name: &str,
    candidate_count: usize,
    kind: ModuleResolutionAmbiguity,
) -> (DiagnosticSeverity, String, &'static str, Vec<(&'static str, String)>) {
    let message_args = || {
        vec![
            ("module_name", module_name.to_owned()),
            ("candidate_count", candidate_count.to_string()),
        ]
    };
    match kind {
        ModuleResolutionAmbiguity::Strict => (
            DiagnosticSeverity::Warning,
            format!(
                "module instantiation '{module_name}' matches {candidate_count} module definitions; cannot determine which one to use"
            ),
            DIAGNOSTIC_AMBIGUOUS_MODULE_STRICT,
            message_args(),
        ),
        ModuleResolutionAmbiguity::BestEffortTie => (
            DiagnosticSeverity::Note,
            format!(
                "module instantiation '{module_name}' matches {candidate_count} module definitions; cannot determine which one to use"
            ),
            DIAGNOSTIC_AMBIGUOUS_MODULE_BEST_EFFORT,
            message_args(),
        ),
    }
}

fn to_text_range(diag: &SyntaxDiagnostic) -> TextRange {
    fn to_text_size(value: usize) -> TextSize {
        let raw = u32::try_from(value).unwrap_or(u32::MAX);
        TextSize::new(raw)
    }

    if let Some(range) = diag.primary_range.as_ref() {
        TextRange::new(to_text_size(range.start), to_text_size(range.end))
    } else if let Some(offset) = diag.location {
        let pos = to_text_size(offset);
        TextRange::new(pos, pos)
    } else {
        TextRange::empty(TextSize::new(0))
    }
}

#[cfg(test)]
mod tests {
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_db::SourceRootDb,
        source_root::{SourceRoot, SourceRootId, SourceRootRole},
    };
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::{lines::LineEnding, paths::AbsPathBuf, test_support::TestDir};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{
        AMBIGUOUS_MODULE_INSTANTIATION, DiagnosticSource, diagnostics, source_root_diagnostics,
    };

    fn db_with_files(files: &[(&str, &str)], configured: bool) -> RootDb {
        db_with_files_in_role(files, SourceRootRole::Local, configured)
    }

    fn db_with_files_in_role(
        files: &[(&str, &str)],
        role: SourceRootRole,
        configured: bool,
    ) -> RootDb {
        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        let mut change = Change::new();

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            let path = VfsPath::new_virtual_path((*path).to_owned());
            file_set.insert(file_id, path);
            change.add_changed_file(ChangedFile {
                file_id,
                change_kind: ChangeKind::Create(Arc::from(*text), LineEnding::Unix),
            });
        }

        change.set_roots(vec![SourceRoot::new(role, file_set)]);
        if configured {
            change.set_project_config(Arc::new(ProjectConfig::new(
                vec![Some(CompilationProfileId(0))],
                vec![CompilationProfile {
                    source_roots: vec![SourceRootId(0)],
                    top_modules: Vec::new(),
                    preprocess: PreprocessConfig::default(),
                }],
            )));
        }
        db.apply_change(change);
        db
    }

    #[test]
    fn best_effort_ambiguous_module_instantiation_reports_vizsla_information() {
        let db = db_with_files_in_role(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", "module top; child u(); endmodule\n"),
            ],
            SourceRootRole::BestEffortIndex,
            false,
        );

        let diagnostics = diagnostics(&db, FileId(2));

        assert!(
            diagnostics.iter().any(|diag| {
                diag.source == DiagnosticSource::Vizsla
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
                    && diag.severity == syntax::DiagnosticSeverity::Note
                    && diag.message.contains("matches 2 module definitions")
            }),
            "expected vizsla ambiguous module information: {diagnostics:?}"
        );
    }

    #[test]
    fn best_effort_nearest_module_instantiation_does_not_report_vizsla_diagnostic() {
        let db = db_with_files_in_role(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/a/top.sv", "module top; child u(); endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
            ],
            SourceRootRole::BestEffortIndex,
            false,
        );

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().all(|diag| diag.source != DiagnosticSource::Vizsla),
            "nearest best-effort module should not produce Vizsla diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn strict_ambiguous_module_instantiation_reports_vizsla_warning() {
        let db = db_with_files(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", "module top; child u(); endmodule\n"),
            ],
            false,
        );

        let diagnostics = diagnostics(&db, FileId(2));

        assert!(
            diagnostics.iter().any(|diag| {
                diag.source == DiagnosticSource::Vizsla
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
                    && diag.severity == syntax::DiagnosticSeverity::Warning
                    && diag.message.contains("matches 2 module definitions")
            }),
            "expected strict ambiguity warning: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_suppress_vizsla_ambiguous_module_warning() {
        let db = db_with_files(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/a/top.sv", "module top; child u(); endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
            ],
            true,
        );

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().all(|diag| diag.source != DiagnosticSource::Vizsla),
            "vizsla ambiguity warning should not duplicate active slang semantic diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_include_other_workspace_files() {
        let db = db_with_files(
            &[
                ("/child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
                ("/top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
            ],
            true,
        );

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
            "expected semantic diagnostic from module declared in another file: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "document diagnostics should only include diagnostics attributed to the requested file: {diagnostics:?}"
        );
        assert!(
            db.semantic_diagnostics(FileId(0)).is_empty(),
            "child file should not receive diagnostics that belong to top.sv"
        );
    }

    #[test]
    fn unconfigured_root_keeps_only_parse_diagnostics() {
        let db = db_with_files(
            &[
                ("/child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
                ("/top.sv", "module top(;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
            ],
            false,
        );

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(!diagnostics.is_empty(), "expected syntax diagnostics: {diagnostics:?}");
        assert!(
            diagnostics.iter().all(|diag| !diag.message.contains("port 'b' has no connection")),
            "unconfigured roots should not run semantic diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn ignored_root_disables_document_diagnostics() {
        let db = db_with_files_in_role(
            &[("/ignored.sv", "module ignored(;\nendmodule\n")],
            SourceRootRole::Ignored,
            true,
        );

        let diagnostics = diagnostics(&db, FileId(0));

        assert!(
            diagnostics.is_empty(),
            "ignored roots must not produce diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn best_effort_index_root_does_not_produce_fallback_compilation_plan() {
        let mut db = RootDb::new(None);
        let file_id = FileId(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_best_effort_index(file_set)]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from("module top; endmodule\n"), LineEnding::Unix),
        });
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));

        assert!(plan.source_roots.is_empty());
        assert!(plan.roots.is_empty());
    }

    #[test]
    fn semantic_diagnostics_map_include_header_files() {
        let root = if cfg!(windows) {
            "C:/vizsla-diagnostics-include"
        } else {
            "/vizsla-diagnostics-include"
        };
        let root = AbsPathBuf::assert(root.into());
        let top_path = root.join("top.sv");
        let header_path = root.join("defs.vh");

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(top_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(header_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(
                Arc::from("module top;\n`include \"defs.vh\"\nendmodule\n"),
                LineEnding::Unix,
            ),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(
                Arc::from("logic value = missing_name;\n"),
                LineEnding::Unix,
            ),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig { predefines: Vec::new(), include_dirs: vec![root] },
            }],
        )));
        db.apply_change(change);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("missing_name")),
            "expected semantic diagnostic in included header: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "header diagnostics should be attributed to the header file: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_do_not_compile_included_sv_as_root_source() {
        let dir = TestDir::new("diagnostics-included-sv");
        let root = dir.path().to_path_buf();
        let pkg_path = root.join("a_pkg.sv");
        let frag_path = root.join("z_frag.sv");
        let pkg_text = "module pkg_mod;\n`include \"z_frag.sv\"\nendmodule\n";
        let disk_frag_text = "logic value = 1'b0;\n";
        let vfs_frag_text = "logic value = missing_name;\n";
        std::fs::write(&pkg_path, pkg_text).unwrap();
        std::fs::write(&frag_path, disk_frag_text).unwrap();

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(pkg_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(frag_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(pkg_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(vfs_frag_text), LineEnding::Unix),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::default(),
            }],
        )));
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert!(plan.include_only.contains(&FileId(1)));
        assert_eq!(plan.roots, vec![FileId(0)]);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.file_id == FileId(1) && diag.message.contains("missing_name")),
            "included .sv should use VFS text and receive mapped diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_follow_transitive_included_sv_buffers() {
        let dir = TestDir::new("diagnostics-transitive-included-sv");
        let src_root = dir.join("src");
        let include_root = dir.join("include");
        std::fs::create_dir_all(&src_root).unwrap();
        std::fs::create_dir_all(&include_root).unwrap();

        let top_path = src_root.join("top.sv");
        let mid_path = include_root.join("mid.sv");
        let leaf_path = include_root.join("leaf.sv");
        let top_text = "module top;\n`include \"mid.sv\"\nendmodule\n";
        let mid_text = "`include \"leaf.sv\"\n";
        let disk_leaf_text = "logic value = 1'b0;\n";
        let vfs_leaf_text = "logic value = missing_name;\n";
        std::fs::write(&top_path, top_text).unwrap();
        std::fs::write(&mid_path, mid_text).unwrap();
        std::fs::write(&leaf_path, disk_leaf_text).unwrap();

        let mut db = RootDb::new(None);
        let mut src_files = FileSet::default();
        src_files.insert(FileId(0), VfsPath::from(top_path));
        let mut include_files = FileSet::default();
        include_files.insert(FileId(1), VfsPath::from(mid_path));
        include_files.insert(FileId(2), VfsPath::from(leaf_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(top_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(mid_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(2),
            change_kind: ChangeKind::Create(Arc::from(vfs_leaf_text), LineEnding::Unix),
        });
        change.set_roots(vec![
            SourceRoot::new_local(src_files),
            SourceRoot::new_local(include_files),
        ]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0)), None],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    predefines: Vec::new(),
                    include_dirs: vec![include_root],
                },
            }],
        )));
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert_eq!(plan.include_only.len(), 2);
        assert!(plan.include_only.contains(&FileId(1)));
        assert!(plan.include_only.contains(&FileId(2)));

        let diagnostics = source_root_diagnostics(&db, FileId(0));

        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.file_id == FileId(2) && diag.message.contains("missing_name")),
            "transitively included .sv should use VFS text: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_compilation_preloads_root_source_buffers() {
        let dir = TestDir::new("diagnostics-preloaded-roots");
        let root = dir.path().to_path_buf();
        let a_path = root.join("a.sv");
        let b_path = root.join("b.sv");
        let a_text = "module a; endmodule\n";
        let b_text = "module b; endmodule\n";
        std::fs::write(&a_path, a_text).unwrap();
        std::fs::write(&b_path, b_text).unwrap();

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(a_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(b_path.clone()));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(a_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(b_text), LineEnding::Unix),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert_eq!(plan.roots, vec![FileId(0), FileId(1)]);
        let buffers = base_db::compilation_plan::compilation_source_buffers_for_plan(&db, &plan);
        let buffer_paths = buffers.iter().map(|buffer| buffer.path.as_str()).collect::<Vec<_>>();
        let a_path = a_path.to_string();
        let b_path = b_path.to_string();
        assert!(buffer_paths.contains(&a_path.as_str()));
        assert!(buffer_paths.contains(&b_path.as_str()));
    }
}
