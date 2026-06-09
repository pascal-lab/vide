use hir::{
    base_db::{
        diagnostics_config::DiagnosticSource as SlangDiagnosticSource,
        project::CompilationProfileId,
        source_db::{SourceDb, SourceRootDb},
        source_root::{SourceRootDiagnosticScope, SourceRootRole},
    },
    db::HirDb,
    hir_def::module::ModuleId,
    source_map::IsSrc,
};
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::{
    get::Get,
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    module_resolution::{ModuleResolution, ModuleResolutionAmbiguity, resolve_module_name},
};

const AMBIGUOUS_MODULE_INSTANTIATION: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 1, subsystem: 0, name: "ambiguous-module-instantiation" };
const INACTIVE_PREPROCESSOR_BRANCH: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 2, subsystem: 0, name: "inactive-preprocessor-branch" };
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_STRICT: &str = "diagnostic.ambiguous_module.strict";
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_BEST_EFFORT: &str = "diagnostic.ambiguous_module.best_effort";
pub const DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH: &str = "diagnostic.inactive_preprocessor_branch";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    SlangParse,
    SlangSemantic,
    Vide,
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
    pub args: Vec<String>,
    pub message_key: Option<&'static str>,
    pub message_args: Vec<(&'static str, String)>,
    pub tags: Vec<DiagnosticTag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticTag {
    Unnecessary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VideDiagnosticDescriptor {
    code: u16,
    subsystem: u16,
    name: &'static str,
}

#[derive(Debug, Clone, Default)]
struct VideDiagnosticMetadata {
    message_args: Vec<(&'static str, String)>,
    tags: Vec<DiagnosticTag>,
}

impl VideDiagnosticDescriptor {
    fn diagnostic(
        self,
        file_id: FileId,
        range: TextRange,
        severity: DiagnosticSeverity,
        message: String,
        message_key: &'static str,
        message_args: Vec<(&'static str, String)>,
    ) -> Diagnostic {
        self.diagnostic_with_metadata(
            file_id,
            range,
            severity,
            message,
            message_key,
            VideDiagnosticMetadata { message_args, tags: Vec::new() },
        )
    }

    fn diagnostic_with_tags(
        self,
        file_id: FileId,
        range: TextRange,
        severity: DiagnosticSeverity,
        message: String,
        message_key: &'static str,
        tags: Vec<DiagnosticTag>,
    ) -> Diagnostic {
        self.diagnostic_with_metadata(
            file_id,
            range,
            severity,
            message,
            message_key,
            VideDiagnosticMetadata { message_args: Vec::new(), tags },
        )
    }

    fn diagnostic_with_metadata(
        self,
        file_id: FileId,
        range: TextRange,
        severity: DiagnosticSeverity,
        message: String,
        message_key: &'static str,
        metadata: VideDiagnosticMetadata,
    ) -> Diagnostic {
        Diagnostic {
            file_id,
            code: self.code,
            subsystem: self.subsystem,
            name: self.name.to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: DiagnosticSource::Vide,
            range,
            severity,
            message,
            args: Vec::new(),
            message_key: Some(message_key),
            message_args: metadata.message_args,
            tags: metadata.tags,
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
    let mut diagnostics = db
        .compilation_profile_diagnostics(profile_id)
        .iter()
        .map(|diag| slang_diagnostic(diag.file_id, diag.source, &diag.diagnostic))
        .collect::<Vec<_>>();

    diagnostics.extend(
        compilation_profile_file_ids(db, profile_id)
            .into_iter()
            .flat_map(|file_id| inactive_preprocessor_branch_diagnostics(db, file_id)),
    );
    diagnostics
}

pub(crate) fn compilation_profile_syntax_diagnostics(
    db: &RootDb,
    profile_id: CompilationProfileId,
) -> Vec<Diagnostic> {
    compilation_profile_file_ids(db, profile_id)
        .into_iter()
        .flat_map(|file_id| syntax_diagnostics(db, file_id))
        .collect()
}

fn compilation_profile_file_ids(db: &RootDb, profile_id: CompilationProfileId) -> Vec<FileId> {
    let plan = db.compilation_plan_for_profile(Some(profile_id));
    let mut file_ids = plan.roots.clone();
    file_ids.extend(plan.include_only.iter().copied());
    file_ids.sort_unstable_by_key(|file_id| file_id.0);
    file_ids.dedup();
    file_ids
}

fn syntax_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = parse_diagnostics(db, file_id);
    diagnostics.extend(vide_diagnostics(db, file_id));
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
        args: diag.args.clone(),
        message_key: None,
        message_args: Vec::new(),
        tags: Vec::new(),
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
        inactive_preprocessor_branch_diagnostics(db, file_id)
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
        diagnostics.extend(
            source_root
                .iter()
                .flat_map(|file_id| inactive_preprocessor_branch_diagnostics(db, file_id)),
        );
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

fn vide_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if !vide_diagnostics_enabled(db) {
        return Vec::new();
    }

    let mut diagnostics = inactive_preprocessor_branch_diagnostics(db, file_id);

    if !slang_semantic_diagnostics_active(db, file_id) {
        diagnostics.extend(module_instantiation_resolution_diagnostics(db, file_id));
    }

    diagnostics
}

fn vide_diagnostics_enabled(db: &RootDb) -> bool {
    db.diagnostics_config().enabled
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
            let Some(src) = src_map.get(instantiation_id) else {
                continue;
            };
            let mut diag_file_id = file_id;
            let mut range = src.expanded_range();
            match hir::preproc::diagnostic_provenance_for_range(db, file_id, range) {
                Ok(Some(provenance)) => {
                    let Some((target_file_id, target_range)) =
                        diagnostic_preproc_target_file_range(&provenance)
                    else {
                        continue;
                    };
                    diag_file_id = target_file_id;
                    range = target_range;
                }
                Ok(None) => {}
                Err(_) => continue,
            }

            match resolve_module_name(db, file_id, module_name) {
                ModuleResolution::Ambiguous { candidates, kind } => {
                    let (severity, message, message_key, message_args) =
                        ambiguous_module_instantiation_diagnostic(
                            module_name,
                            candidates.len(),
                            kind,
                        );
                    diagnostics.push(AMBIGUOUS_MODULE_INSTANTIATION.diagnostic(
                        diag_file_id,
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

fn diagnostic_preproc_target_file_range(
    provenance: &hir::preproc::DiagnosticProvenance,
) -> Option<(FileId, TextRange)> {
    match provenance {
        hir::preproc::DiagnosticProvenance::SourceToken { source, range }
        | hir::preproc::DiagnosticProvenance::MacroBody { source, range, .. }
        | hir::preproc::DiagnosticProvenance::MacroArgument { source, range, .. }
        | hir::preproc::DiagnosticProvenance::VirtualExpansion { source, range } => {
            Some((source.file_id()?, *range))
        }
        hir::preproc::DiagnosticProvenance::Builtin { call, .. } => {
            Some((call.file_id, call.range))
        }
        hir::preproc::DiagnosticProvenance::Unavailable(_) => None,
    }
}

fn inactive_preprocessor_branch_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if !vide_diagnostics_enabled(db) {
        return Vec::new();
    }

    hir::preproc::inactive_branches(db, file_id)
        .unwrap_or_default()
        .iter()
        .map(|branch| {
            INACTIVE_PREPROCESSOR_BRANCH.diagnostic_with_tags(
                branch.file_id,
                branch.range,
                DiagnosticSeverity::Note,
                "code is inactive due to preprocessor conditionals".to_owned(),
                DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH,
                vec![DiagnosticTag::Unnecessary],
            )
        })
        .collect()
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
    use hir::base_db::{
        change::Change,
        compilation_plan::compilation_source_buffers_for_plan,
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::Durability,
        source_db::{PreprocVirtualOrigin, SourceDb, SourceRootDb},
        source_root::{SourceRoot, SourceRootId, SourceRootRole},
    };
    use triomphe::Arc;
    use utils::{
        line_index::{TextRange, TextSize},
        lines::LineEnding,
        paths::AbsPathBuf,
        test_support::TestDir,
    };
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{
        AMBIGUOUS_MODULE_INSTANTIATION, DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH, DiagnosticSource,
        DiagnosticTag, INACTIVE_PREPROCESSOR_BRANCH, diagnostic_preproc_target_file_range,
        diagnostics, source_root_diagnostics,
    };
    use crate::db::root_db::RootDb;

    fn db_with_files(files: &[(&str, &str)], configured: bool) -> RootDb {
        db_with_files_in_role(files, SourceRootRole::Local, configured)
    }

    fn db_with_predefines(files: &[(&str, &str)], predefines: Vec<String>) -> RootDb {
        db_with_files_in_role_and_preprocess(
            files,
            SourceRootRole::Local,
            true,
            PreprocessConfig::with_predefine_strings(predefines, Vec::new()),
        )
    }

    fn disable_diagnostics(db: &mut RootDb) {
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig { enabled: false, ..DiagnosticsConfig::default() }),
            Durability::HIGH,
        );
    }

    fn disable_semantic_diagnostics(db: &mut RootDb) {
        let mut config = DiagnosticsConfig::default();
        config.semantic.enabled = false;
        db.set_diagnostics_config_with_durability(Arc::new(config), Durability::HIGH);
    }

    fn db_with_files_in_role(
        files: &[(&str, &str)],
        role: SourceRootRole,
        configured: bool,
    ) -> RootDb {
        db_with_files_in_role_and_preprocess(files, role, configured, PreprocessConfig::default())
    }

    fn db_with_files_in_role_and_preprocess(
        files: &[(&str, &str)],
        role: SourceRootRole,
        configured: bool,
        preprocess: PreprocessConfig,
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
                    preprocess,
                }],
            )));
        }
        db.apply_change(change);
        db
    }

    fn range_of(text: &str, needle: &str) -> TextRange {
        let start = TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap());
        TextRange::new(start, start + TextSize::of(needle))
    }

    #[test]
    fn best_effort_ambiguous_module_instantiation_reports_vide_information() {
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
                diag.source == DiagnosticSource::Vide
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
                    && diag.severity == syntax::DiagnosticSeverity::Note
                    && diag.message.contains("matches 2 module definitions")
            }),
            "expected vide ambiguous module information: {diagnostics:?}"
        );
    }

    #[test]
    fn best_effort_nearest_module_instantiation_does_not_report_vide_diagnostic() {
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
            diagnostics.iter().all(|diag| diag.source != DiagnosticSource::Vide),
            "nearest best-effort module should not produce Vide diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn strict_ambiguous_module_instantiation_reports_vide_warning() {
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
                diag.source == DiagnosticSource::Vide
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
                    && diag.severity == syntax::DiagnosticSeverity::Warning
                    && diag.message.contains("matches 2 module definitions")
            }),
            "expected strict ambiguity warning: {diagnostics:?}"
        );
    }

    #[test]
    fn preproc_macro_generated_instantiation_diagnostic_uses_macro_body_provenance() {
        let top = "`define MAKE child u();\nmodule top;\n  `MAKE\nendmodule\n";
        let db = db_with_files(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", top),
            ],
            false,
        );

        let diagnostics = diagnostics(&db, FileId(2));
        let diagnostic = diagnostics
            .iter()
            .find(|diag| {
                diag.source == DiagnosticSource::Vide
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
            })
            .unwrap_or_else(|| {
                panic!("expected generated instantiation diagnostic: {diagnostics:?}")
            });

        assert_eq!(diagnostic.file_id, FileId(2));
        assert_eq!(diagnostic.range, range_of(top, "child"));
        assert_ne!(diagnostic.range, range_of(top, "`MAKE"));
    }

    #[test]
    fn preproc_display_only_virtual_expansion_diagnostic_is_not_published() {
        let top = "module top;\n  `MAKE\nendmodule\n";
        let mut db = db_with_predefines(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", top),
            ],
            vec!["MAKE=child u();".to_owned()],
        );
        disable_semantic_diagnostics(&mut db);

        let diagnostics = diagnostics(&db, FileId(2));

        assert!(
            diagnostics.iter().all(|diag| {
                diag.source != DiagnosticSource::Vide
                    || diag.name != AMBIGUOUS_MODULE_INSTANTIATION.name
            }),
            "display-only virtual expansion must not publish ambiguous module diagnostics: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id.0 < 3),
            "diagnostics must not target synthetic virtual FileIds: {diagnostics:?}"
        );
    }

    #[test]
    fn diagnostic_target_rejects_display_only_virtual_expansion() {
        let provenance = hir::preproc::DiagnosticProvenance::VirtualExpansion {
            source: hir::preproc::MappedPreprocSource::VirtualDisplay {
                path: VfsPath::new_virtual_path(
                    "/__vide/preproc/profile-0/expansion/0.sv".to_owned(),
                ),
                origin: PreprocVirtualOrigin::Builtin { name: "display-only".into() },
            },
            range: TextRange::new(TextSize::from(0), TextSize::from(5)),
        };

        assert_eq!(diagnostic_preproc_target_file_range(&provenance), None);
    }

    #[test]
    fn diagnostic_target_accepts_materialized_virtual_expansion() {
        let file_id = FileId(7);
        let range = TextRange::new(TextSize::from(0), TextSize::from(5));
        let provenance = hir::preproc::DiagnosticProvenance::VirtualExpansion {
            source: hir::preproc::MappedPreprocSource::VirtualFile {
                file_id,
                path: VfsPath::new_virtual_path(
                    "/__vide/preproc/profile-0/expansion/0.sv".to_owned(),
                ),
                origin: PreprocVirtualOrigin::Builtin { name: "materialized".into() },
            },
            range,
        };

        assert_eq!(diagnostic_preproc_target_file_range(&provenance), Some((file_id, range)));
    }

    #[test]
    fn semantic_diagnostics_suppress_vide_ambiguous_module_warning() {
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
            diagnostics.iter().all(|diag| diag.source != DiagnosticSource::Vide),
            "vide ambiguity warning should not duplicate active slang semantic diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn inactive_preprocessor_branch_reports_unnecessary_hint() {
        let text = "`ifdef USE_IMPL\nlogic if_body;\n`else\nlogic else_body;\n`endif\n";
        let db = db_with_files(&[("/top.sv", text)], false);

        let diagnostics = diagnostics(&db, FileId(0));
        let inactive = diagnostics
            .iter()
            .find(|diag| diag.name == INACTIVE_PREPROCESSOR_BRANCH.name)
            .expect("expected inactive preprocessor branch diagnostic");

        assert_eq!(inactive.severity, syntax::DiagnosticSeverity::Note);
        assert_eq!(inactive.tags, vec![DiagnosticTag::Unnecessary]);
        assert_eq!(inactive.message_key, Some(DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH));
        assert_eq!(inactive.range, range_of(text, "logic if_body;"));
    }

    #[test]
    fn inactive_preprocessor_branch_marks_else_body_when_ifdef_is_defined() {
        let text = "`ifdef USE_IMPL\nlogic if_body;\n`else\nlogic else_body;\n`endif\n";
        let db = db_with_predefines(&[("/top.sv", text)], vec!["USE_IMPL".to_owned()]);

        let diagnostics = diagnostics(&db, FileId(0));
        let inactive = diagnostics
            .iter()
            .find(|diag| diag.name == INACTIVE_PREPROCESSOR_BRANCH.name)
            .expect("expected inactive preprocessor branch diagnostic");

        assert_eq!(inactive.range, range_of(text, "logic else_body;"));
    }

    #[test]
    fn inactive_preprocessor_branch_respects_global_diagnostics_switch() {
        let mut db = db_with_files(
            &[("/top.sv", "`ifdef USE_IMPL\nlogic active;\n`else\nlogic inactive;\n`endif\n")],
            false,
        );
        disable_diagnostics(&mut db);

        let diagnostics = diagnostics(&db, FileId(0));

        assert!(
            diagnostics.is_empty(),
            "global diagnostics switch must suppress Vide inactive diagnostics: {diagnostics:?}"
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
    fn syntax_only_manifest_does_not_disable_open_file_syntax_diagnostics() {
        let manifest_id = FileId(0);
        let open_file_id = FileId(1);
        let mut manifest_files = FileSet::default();
        manifest_files.insert(manifest_id, VfsPath::new_virtual_path("/project/vide.toml".into()));
        let mut open_files = FileSet::default();
        open_files.insert(open_file_id, VfsPath::new_virtual_path("/scratch/open.sv".into()));

        let mut change = Change::new();
        change.set_roots(vec![
            SourceRoot::new_local(manifest_files),
            SourceRoot::new_ignored(open_files),
        ]);
        change.set_project_config(Arc::new(ProjectConfig::new(vec![None, None], Vec::new())));
        change.add_changed_file(ChangedFile {
            file_id: manifest_id,
            change_kind: ChangeKind::Create(Arc::from(""), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: open_file_id,
            change_kind: ChangeKind::Create(
                Arc::from("module open(;\nendmodule\n"),
                LineEnding::Unix,
            ),
        });

        let mut db = RootDb::new(None);
        db.apply_change(change);

        assert!(!db.project_config().has_compilation_profiles());
        assert_eq!(db.project_config().profile_for_root(SourceRootId(0)), None);
        assert_eq!(db.project_config().profile_for_root(SourceRootId(1)), None);
        assert!(diagnostics(&db, manifest_id).is_empty());

        let diagnostics = diagnostics(&db, open_file_id);
        assert!(
            diagnostics.iter().any(|diag| diag.source == DiagnosticSource::SlangParse),
            "profile-less open files should keep syntax diagnostics: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| {
                diag.file_id == open_file_id && diag.source != DiagnosticSource::SlangSemantic
            }),
            "syntax-only manifest must not create semantic diagnostic ownership: {diagnostics:?}"
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
        let root =
            if cfg!(windows) { "C:/vide-diagnostics-include" } else { "/vide-diagnostics-include" };
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
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
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
                    include_dirs: vec![include_root],
                    ..PreprocessConfig::default()
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
        let buffers = compilation_source_buffers_for_plan(&db, &plan);
        let buffer_paths = buffers.iter().map(|buffer| buffer.path.as_str()).collect::<Vec<_>>();
        let a_path = a_path.to_string();
        let b_path = b_path.to_string();
        assert!(buffer_paths.contains(&a_path.as_str()));
        assert!(buffer_paths.contains(&b_path.as_str()));
    }
}
