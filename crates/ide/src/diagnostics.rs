use base_db::{
    diagnostics_config::DiagnosticSource as SlangDiagnosticSource,
    project::CompilationProfileId,
    source_db::{SourceDb, SourceRootDb},
    source_root::{SourceRootDiagnosticScope, SourceRootRole},
};
use hir_def::source_map::{LoweringDiagnostic, LoweringDiagnosticKind};
use syntax::{DiagCode, DiagnosticSeverity, SyntaxDiagnostic};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    module_resolution::{ModuleResolution, ModuleResolutionAmbiguity, resolve_module_name},
};

const AMBIGUOUS_MODULE_INSTANTIATION: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 1, subsystem: 0, name: "ambiguous-module-instantiation" };
const INACTIVE_PREPROCESSOR_BRANCH: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 2, subsystem: 0, name: "inactive-preprocessor-branch" };
const LOWERING_INVALID_SYNTAX: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 3, subsystem: 0, name: "lowering-invalid-syntax" };
const LOWERING_UNSUPPORTED_SYNTAX: VideDiagnosticDescriptor =
    VideDiagnosticDescriptor { code: 4, subsystem: 0, name: "lowering-unsupported-syntax" };
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_STRICT: &str = "diagnostic.ambiguous_module.strict";
pub const DIAGNOSTIC_AMBIGUOUS_MODULE_BEST_EFFORT: &str = "diagnostic.ambiguous_module.best_effort";
pub const DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH: &str = "diagnostic.inactive_preprocessor_branch";
pub const DIAGNOSTIC_LOWERING_INVALID_SYNTAX: &str = "diagnostic.lowering.invalid_syntax";
pub const DIAGNOSTIC_LOWERING_UNSUPPORTED_SYNTAX: &str = "diagnostic.lowering.unsupported_syntax";

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

impl Diagnostic {
    /// The token a parse `ExpectedToken` diagnostic wanted to insert, when
    /// this is such a diagnostic.
    ///
    /// Assumes slang's parse `ExpectedToken` reports the wanted token as the
    /// first diagnostic arg. This is positional: if slang changes that
    /// layout, the `insert_expected_token` quick fix silently inserts the
    /// wrong token, so when upgrading slang, re-check that `args[0]` is still
    /// the expected token (covered by the `insert_expected_token` tests).
    pub(crate) fn expected_token(&self) -> Option<&str> {
        (self.source == DiagnosticSource::SlangParse
            && DiagCode::from_raw(self.subsystem, self.code) == DiagCode::EXPECTED_TOKEN)
            .then(|| self.args.first())
            .flatten()
            .map(String::as_str)
    }
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
    if db.file_kind(file_id).is_project_manifest() {
        return crate::manifest::diagnostics(db, file_id);
    }
    db.parse_diagnostics(file_id)
        .iter()
        .filter_map(|diag| slang_diagnostic(file_id, SlangDiagnosticSource::Parse, diag))
        .collect()
}

pub(crate) fn compilation_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    db.file_compilation_diagnostics(file_id)
        .iter()
        .filter_map(|diag| slang_diagnostic(diag.file_id, diag.source, &diag.diagnostic))
        .collect()
}

pub(crate) fn compilation_profile_diagnostics(
    db: &RootDb,
    profile_id: CompilationProfileId,
) -> Vec<Diagnostic> {
    let mut diagnostics = db
        .compilation_profile_diagnostics(profile_id)
        .diagnostics
        .iter()
        .filter_map(|diag| slang_diagnostic(diag.file_id, diag.source, &diag.diagnostic))
        .collect::<Vec<_>>();

    diagnostics.extend(
        compilation_profile_file_ids(db, profile_id)
            .into_iter()
            .flat_map(|file_id| vide_diagnostics(db, file_id)),
    );
    diagnostics
}

fn compilation_profile_file_ids(db: &RootDb, profile_id: CompilationProfileId) -> Vec<FileId> {
    db.compilation_plan_for_profile(Some(profile_id)).all_file_ids()
}

fn syntax_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if db.file_kind(file_id).is_project_manifest() {
        return crate::manifest::diagnostics(db, file_id);
    }
    let mut diagnostics = parse_diagnostics(db, file_id);
    diagnostics.extend(vide_diagnostics(db, file_id));
    diagnostics
}

fn slang_diagnostic(
    file_id: FileId,
    source: SlangDiagnosticSource,
    diag: &SyntaxDiagnostic,
) -> Option<Diagnostic> {
    let range = to_text_range(diag)?;
    Some(Diagnostic {
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
        range,
        severity: diag.severity,
        message: diag.message.clone(),
        args: diag.args.clone(),
        message_key: None,
        message_args: Vec::new(),
        tags: Vec::new(),
    })
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

    syntax_diagnostics(db, file_id)
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

    source_root.iter().flat_map(|file_id| syntax_diagnostics(db, file_id)).collect()
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

/// A pluggable source of `DiagnosticSource::Vide` diagnostics for a file.
///
/// Each Vide check is a self-contained provider registered in
/// [`vide_providers`]; adding a new check is a new type plus one registration
/// entry instead of another branch in `vide_diagnostics`. `active` lets a
/// provider opt out per file (e.g. when slang's own semantic pass already
/// covers the case, as for ambiguous module instantiations).
trait VideDiagnosticProvider {
    /// Whether this provider should run for `file_id`.
    fn active(&self, _db: &RootDb, _file_id: FileId) -> bool {
        true
    }

    /// Compute this provider's diagnostics for `file_id`.
    fn diagnostic(&self, db: &RootDb, file_id: FileId) -> Vec<Diagnostic>;
}

fn vide_providers() -> Vec<Box<dyn VideDiagnosticProvider>> {
    vec![
        Box::new(InactivePreprocessorBranch),
        Box::new(AmbiguousModuleInstantiation),
        Box::new(LoweringSyntaxDiagnostics),
    ]
}

fn vide_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if !vide_diagnostics_enabled(db) {
        return Vec::new();
    }

    vide_providers()
        .into_iter()
        .filter(|provider| provider.active(db, file_id))
        .flat_map(|provider| provider.diagnostic(db, file_id))
        .collect()
}

fn vide_diagnostics_enabled(db: &RootDb) -> bool {
    db.diagnostics_config().enabled
}

/// HIR lowering recovery diagnostics surfaced to the editor.
///
/// [`hir_def::diagnostics::file_lowering_diagnostics`] already resolved a
/// display range for every diagnostic (including the `range: None` cases the
/// lowerer could not locate), so this conversion only decides severity and
/// deduplicates against slang.
struct LoweringSyntaxDiagnostics;

impl VideDiagnosticProvider for LoweringSyntaxDiagnostics {
    fn diagnostic(&self, db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
        lowering_syntax_diagnostics(db, file_id)
    }
}

fn lowering_syntax_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let parse_ranges =
        db.parse_diagnostics(file_id).iter().filter_map(to_text_range).collect::<Vec<_>>();

    db.file_lowering_diagnostics(file_id.into())
        .iter()
        .filter_map(|diag| lowering_diagnostic(file_id, diag, &parse_ranges))
        .collect()
}

fn lowering_diagnostic(
    file_id: FileId,
    diag: &LoweringDiagnostic,
    parse_ranges: &[TextRange],
) -> Option<Diagnostic> {
    let range = diag.range?;
    let (descriptor, severity, message_key) = match diag.kind {
        LoweringDiagnosticKind::InvalidSyntax => {
            // The parser already flagged the offending syntax at this range;
            // publishing the lowering recovery note on top would duplicate the
            // squiggle. `parse_diagnostics` is config-gated, so disabling
            // parse diagnostics keeps the note as a fallback.
            if parse_ranges.iter().any(|parse_range| ranges_overlap(*parse_range, range)) {
                return None;
            }
            (LOWERING_INVALID_SYNTAX, DiagnosticSeverity::Note, DIAGNOSTIC_LOWERING_INVALID_SYNTAX)
        }
        LoweringDiagnosticKind::UnsupportedSyntax => (
            // Valid SystemVerilog that vide does not lower yet; slang has no
            // diagnostic for it, so this is the only signal the user gets.
            LOWERING_UNSUPPORTED_SYNTAX,
            DiagnosticSeverity::Warning,
            DIAGNOSTIC_LOWERING_UNSUPPORTED_SYNTAX,
        ),
    };
    let syntax_kind = format!("{:?}", diag.syntax_kind);
    let kind_label = match diag.kind {
        LoweringDiagnosticKind::InvalidSyntax => "invalid",
        LoweringDiagnosticKind::UnsupportedSyntax => "unsupported",
    };
    let message = format!("{kind_label} syntax '{syntax_kind}': {}", diag.message);
    Some(descriptor.diagnostic(
        file_id,
        range,
        severity,
        message,
        message_key,
        vec![("syntax_kind", syntax_kind), ("message", diag.message.to_string())],
    ))
}

fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    a.start() <= b.end() && b.start() <= a.end()
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
    let hir_file = db.body(db.owner_table(hir_file_id).file_owner().expect("file owner"));
    let mut diagnostics = Vec::new();

    for module_id in hir_file.module_owners() {
        let module = db.body_with_source_map(module_id);
        for (instantiation_id, instantiation) in module.instantiations.iter() {
            let Some(module_name) = instantiation.module_name.as_ref() else {
                continue;
            };
            let mut diag_file_id = file_id;
            let Some(mut range) = module.source_range(db, instantiation_id) else {
                continue;
            };
            match preproc_expand::preproc::diagnostic_target_for_range(db, file_id, range) {
                Ok(result) => {
                    if let Some(target) = result.target {
                        diag_file_id = target.file_id;
                        range = target.range;
                    } else if result.covered {
                        continue;
                    }
                }
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

fn inactive_preprocessor_branch_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    if !vide_diagnostics_enabled(db) {
        return Vec::new();
    }

    let branches = match preproc_expand::preproc::inactive_branches(db, file_id) {
        Ok(branches) => branches,
        Err(error) => {
            tracing::warn!(
                ?error,
                ?file_id,
                "inactive preprocessor branch diagnostics unavailable"
            );
            return Vec::new();
        }
    };
    branches
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

struct InactivePreprocessorBranch;

impl VideDiagnosticProvider for InactivePreprocessorBranch {
    fn diagnostic(&self, db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
        inactive_preprocessor_branch_diagnostics(db, file_id)
    }
}

struct AmbiguousModuleInstantiation;

impl VideDiagnosticProvider for AmbiguousModuleInstantiation {
    fn active(&self, db: &RootDb, file_id: FileId) -> bool {
        !slang_semantic_diagnostics_active(db, file_id)
    }

    fn diagnostic(&self, db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
        module_instantiation_resolution_diagnostics(db, file_id)
    }
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

fn to_text_range(diag: &SyntaxDiagnostic) -> Option<TextRange> {
    fn to_text_size(value: usize) -> Option<TextSize> {
        Some(TextSize::new(u32::try_from(value).ok()?))
    }

    if let Some(range) = diag.primary_range.as_ref() {
        let start = to_text_size(range.start)?;
        let end = to_text_size(range.end)?;
        return (start <= end).then(|| TextRange::new(start, end));
    } else if let Some(offset) = diag.location {
        let pos = to_text_size(offset)?;
        return Some(TextRange::empty(pos));
    }

    tracing::debug!(
        code = diag.code,
        subsystem = diag.subsystem,
        name = %diag.name,
        "dropping Slang diagnostic without a source location"
    );
    None
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use base_db::{
        change::Change,
        diagnostics_config::{DiagnosticPhaseConfig, DiagnosticsConfig},
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::Durability,
        source_db::SourceDb,
        source_root::{SourceRoot, SourceRootId, SourceRootRole},
    };
    use preproc_expand::compilation_plan::compilation_source_buffers_for_plan;
    use triomphe::Arc;
    use utils::{
        line_index::{TextRange, TextSize},
        paths::AbsPathBuf,
        test_support::TestDir,
    };
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::{
        AMBIGUOUS_MODULE_INSTANTIATION, DIAGNOSTIC_INACTIVE_PREPROCESSOR_BRANCH,
        DIAGNOSTIC_LOWERING_INVALID_SYNTAX, DiagnosticSource, DiagnosticTag,
        INACTIVE_PREPROCESSOR_BRANCH, LOWERING_INVALID_SYNTAX, LOWERING_UNSUPPORTED_SYNTAX,
        SlangDiagnosticSource, SyntaxDiagnostic, compilation_profile_diagnostics, diagnostics,
        slang_diagnostic, to_text_range,
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
            let file_id = FileId::from_raw(idx as u32);
            let path = VfsPath::new_virtual_path((*path).to_owned());
            file_set.insert(file_id, path);
            change.add_changed_file(ChangedFile::create(file_id, *text));
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

        let diagnostics = diagnostics(&db, FileId::from_raw(2));

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

        let diagnostics = diagnostics(&db, FileId::from_raw(1));

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

        let diagnostics = diagnostics(&db, FileId::from_raw(2));

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
    fn preproc_macro_generated_instantiation_diagnostic_uses_macro_body_target() {
        let top = "`define MAKE child u();\nmodule top;\n  `MAKE\nendmodule\n";
        let db = db_with_files(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
                ("/project/top.sv", top),
            ],
            false,
        );

        let diagnostics = diagnostics(&db, FileId::from_raw(2));
        let diagnostic = diagnostics
            .iter()
            .find(|diag| {
                diag.source == DiagnosticSource::Vide
                    && diag.name == AMBIGUOUS_MODULE_INSTANTIATION.name
            })
            .unwrap_or_else(|| {
                panic!("expected generated instantiation diagnostic: {diagnostics:?}")
            });

        assert_eq!(diagnostic.file_id, FileId::from_raw(2));
        assert_eq!(diagnostic.range, range_of(top, "child"));
        assert_ne!(diagnostic.range, range_of(top, "`MAKE"));
    }

    #[test]
    fn preproc_display_only_generated_diagnostic_is_not_published() {
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

        let diagnostics = diagnostics(&db, FileId::from_raw(2));

        assert!(
            diagnostics.iter().all(|diag| {
                diag.source != DiagnosticSource::Vide
                    || diag.name != AMBIGUOUS_MODULE_INSTANTIATION.name
            }),
            "display-only virtual expansion must not publish ambiguous module diagnostics: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id.index() < 3),
            "diagnostics must not target synthetic virtual FileIds: {diagnostics:?}"
        );
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

        let diagnostics = diagnostics(&db, FileId::from_raw(1));

        assert!(
            diagnostics.iter().all(|diag| diag.source != DiagnosticSource::Vide),
            "vide ambiguity warning should not duplicate active slang semantic diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn compilation_profile_diagnostics_include_vide_diagnostics() {
        let mut db = db_with_files(
            &[
                ("/project/a/child.sv", "module child; endmodule\n"),
                ("/project/a/top.sv", "module top; child u(); endmodule\n"),
                ("/project/b/child.sv", "module child; endmodule\n"),
            ],
            true,
        );
        disable_semantic_diagnostics(&mut db);

        let diagnostics = compilation_profile_diagnostics(&db, CompilationProfileId(0));

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.file_id == FileId::from_raw(1)
                    && diagnostic.source == DiagnosticSource::Vide
                    && diagnostic.name == AMBIGUOUS_MODULE_INSTANTIATION.name
            }),
            "profile diagnostics should include Vide diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn inactive_preprocessor_branch_reports_unnecessary_hint() {
        let text = "`ifdef USE_IMPL\nlogic if_body;\n`else\nlogic else_body;\n`endif\n";
        let db = db_with_files(&[("/top.sv", text)], false);

        let diagnostics = diagnostics(&db, FileId::from_raw(0));
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

        let diagnostics = diagnostics(&db, FileId::from_raw(0));
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

        let diagnostics = diagnostics(&db, FileId::from_raw(0));

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

        let diagnostics = compilation_profile_diagnostics(&db, CompilationProfileId(0));

        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.message.contains("input port 'b' has no connection")),
            "expected semantic diagnostic from module declared in another file: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId::from_raw(1)),
            "document diagnostics should only include diagnostics attributed to the requested file: {diagnostics:?}"
        );
        assert!(
            db.semantic_diagnostics(FileId::from_raw(0)).is_empty(),
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

        let diagnostics = diagnostics(&db, FileId::from_raw(1));

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

        let diagnostics = diagnostics(&db, FileId::from_raw(0));

        assert!(
            diagnostics.is_empty(),
            "ignored roots must not produce diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn syntax_only_manifest_does_not_disable_open_file_syntax_diagnostics() {
        let manifest_id = FileId::from_raw(0);
        let open_file_id = FileId::from_raw(1);
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
        change.add_changed_file(ChangedFile::create(manifest_id, ""));
        change.add_changed_file(ChangedFile::create(open_file_id, "module open(;\nendmodule\n"));

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
        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_best_effort_index(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, "module top; endmodule\n"));
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
        file_set.insert(FileId::from_raw(0), VfsPath::from(top_path.clone()));
        file_set.insert(FileId::from_raw(1), VfsPath::from(header_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(
            FileId::from_raw(0),
            "module top;\n`include \"defs.vh\"\nendmodule\n",
        ));
        change.add_changed_file(ChangedFile::create(
            FileId::from_raw(1),
            "logic value;\nlogic value;\n",
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
        db.apply_change(change);

        let diagnostics = compilation_profile_diagnostics(&db, CompilationProfileId(0));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("redefinition of 'value'")),
            "expected semantic diagnostic in included header: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId::from_raw(1)),
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
        let disk_frag_text = "logic value;\n";
        let vfs_frag_text = "logic value;\nlogic value;\n";
        std::fs::write(&pkg_path, pkg_text).unwrap();
        std::fs::write(&frag_path, disk_frag_text).unwrap();

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId::from_raw(0), VfsPath::from(pkg_path.clone()));
        file_set.insert(FileId::from_raw(1), VfsPath::from(frag_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(FileId::from_raw(0), pkg_text));
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), vfs_frag_text));
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
        assert!(plan.include_only.contains(&FileId::from_raw(1)));
        assert_eq!(plan.roots, vec![FileId::from_raw(0)]);

        let diagnostics = compilation_profile_diagnostics(&db, CompilationProfileId(0));

        assert!(
            diagnostics.iter().any(|diag| diag.file_id == FileId::from_raw(1)
                && diag.message.contains("redefinition of 'value'")),
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
        let disk_leaf_text = "logic value;\n";
        let vfs_leaf_text = "logic value;\nlogic value;\n";
        std::fs::write(&top_path, top_text).unwrap();
        std::fs::write(&mid_path, mid_text).unwrap();
        std::fs::write(&leaf_path, disk_leaf_text).unwrap();

        let mut db = RootDb::new(None);
        let mut src_files = FileSet::default();
        src_files.insert(FileId::from_raw(0), VfsPath::from(top_path));
        let mut include_files = FileSet::default();
        include_files.insert(FileId::from_raw(1), VfsPath::from(mid_path));
        include_files.insert(FileId::from_raw(2), VfsPath::from(leaf_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(FileId::from_raw(0), top_text));
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), mid_text));
        change.add_changed_file(ChangedFile::create(FileId::from_raw(2), vfs_leaf_text));
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
        assert!(plan.include_only.contains(&FileId::from_raw(1)));
        assert!(plan.include_only.contains(&FileId::from_raw(2)));

        let diagnostics = compilation_profile_diagnostics(&db, CompilationProfileId(0));

        assert!(
            diagnostics.iter().any(|diag| diag.file_id == FileId::from_raw(2)
                && diag.message.contains("redefinition of 'value'")),
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
        file_set.insert(FileId::from_raw(0), VfsPath::from(a_path.clone()));
        file_set.insert(FileId::from_raw(1), VfsPath::from(b_path.clone()));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(FileId::from_raw(0), a_text));
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), b_text));
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert_eq!(plan.roots, vec![FileId::from_raw(0), FileId::from_raw(1)]);
        let buffers = compilation_source_buffers_for_plan(&db, &plan);
        let buffer_paths = buffers.iter().map(|buffer| buffer.path.as_str()).collect::<Vec<_>>();
        let a_path = a_path.to_string();
        let b_path = b_path.to_string();
        assert!(buffer_paths.contains(&a_path.as_str()));
        assert!(buffer_paths.contains(&b_path.as_str()));
    }

    #[test]
    fn lowered_assignment_pattern_has_no_vide_warning() {
        let text = "module m;\n  int x = '{default: 0};\nendmodule\n";
        let db = db_with_files(&[("/top.sv", text)], false);

        let diagnostics = diagnostics(&db, FileId::from_raw(0));
        assert!(
            !diagnostics.iter().any(|diag| diag.name == LOWERING_UNSUPPORTED_SYNTAX.name),
            "supported assignment patterns must not produce lowering warnings: {diagnostics:?}"
        );
    }

    /// `for (i = 0; i < 1; 2)` — the iteration expression `2` is not a valid
    /// genvar iteration form, so slang parses it into a `BadExpression` (with
    /// a parse diagnostic) and lowering reports it as invalid syntax.
    const INVALID_ITERATION_TEXT: &str =
        "module m;\n  genvar i;\n  for (i = 0; i < 1; 2) begin : g\n  end\nendmodule\n";

    #[test]
    fn lowering_invalid_syntax_is_suppressed_by_parse_diagnostic() {
        let db = db_with_files(&[("/top.sv", INVALID_ITERATION_TEXT)], false);

        let diagnostics = diagnostics(&db, FileId::from_raw(0));

        assert!(
            diagnostics.iter().any(|diag| diag.source == DiagnosticSource::SlangParse),
            "the fixture must produce a parse diagnostic: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.name != LOWERING_INVALID_SYNTAX.name),
            "lowering invalid-syntax must not duplicate the parse diagnostic: {diagnostics:?}"
        );
    }

    #[test]
    fn lowering_invalid_syntax_reports_note_when_parse_diagnostics_disabled() {
        let mut db = db_with_files(&[("/top.sv", INVALID_ITERATION_TEXT)], false);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig {
                parse: DiagnosticPhaseConfig { enabled: false },
                ..DiagnosticsConfig::default()
            }),
            Durability::HIGH,
        );

        let diagnostics = diagnostics(&db, FileId::from_raw(0));
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.name == LOWERING_INVALID_SYNTAX.name)
            .unwrap_or_else(|| {
                panic!("expected lowering invalid-syntax note fallback: {diagnostics:?}")
            });

        assert_eq!(diagnostic.source, DiagnosticSource::Vide);
        assert_eq!(diagnostic.severity, syntax::DiagnosticSeverity::Note);
        assert_eq!(diagnostic.message_key, Some(DIAGNOSTIC_LOWERING_INVALID_SYNTAX));
        assert_eq!(diagnostic.range, range_of(INVALID_ITERATION_TEXT, "2"));
    }

    #[test]
    fn lowering_diagnostics_lsp_snapshot() {
        let text = r#"
module lowering_diags;
  int pattern = '{default: 0};
  struct { logic a; } struct_value;
  genvar i;
  for (i = 0; i < 1; 2) begin : g
  end
  initial begin : blk
    int block_pattern = '{default: 0};
  end
  task automatic t;
    int task_pattern = '{default: 0};
  endtask
endmodule
"#;
        let db = db_with_files(&[("/top.sv", text)], false);

        let mut report = String::new();
        for diag in diagnostics(&db, FileId::from_raw(0)) {
            writeln!(
                &mut report,
                "{:?} {} {:?} {:?} key={:?} args={:?} {}",
                diag.source,
                diag.name,
                diag.severity,
                diag.range,
                diag.message_key,
                diag.message_args,
                diag.message
            )
            .unwrap();
        }
        insta::assert_snapshot!("lowering_diagnostics_lsp_snapshot", report);
    }

    #[test]
    fn unlocated_slang_diagnostics_are_not_published_at_file_start() {
        let diagnostic = SyntaxDiagnostic {
            code: 1,
            subsystem: 5,
            severity: syntax::DiagnosticSeverity::Error,
            message: "global diagnostic".to_owned(),
            args: Vec::new(),
            name: "GlobalDiagnostic".to_owned(),
            option_name: None,
            groups: Vec::new(),
            primary_range: None,
            location: None,
            buffer_id: None,
            file_name: None,
        };

        assert!(to_text_range(&diagnostic).is_none());
        assert!(
            slang_diagnostic(FileId::from_raw(0), SlangDiagnosticSource::Parse, &diagnostic)
                .is_none()
        );
    }
}
