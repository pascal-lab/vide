use std::ops::Range;

use base_db::{
    diagnostics_config::{
        DiagnosticRuleSeverity, DiagnosticSelector, DiagnosticSource, DiagnosticsConfig,
    },
    project::CompilationProfileId,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use syntax::{
    SyntaxTreeBuffer, SyntaxTreeOptions,
    compilation::Compilation,
    diagnostics::{
        DiagnosticSeverity, SyntaxDiagnostic, SyntaxDiagnosticExpansion, SyntaxDiagnosticLocation,
        SyntaxDiagnosticRange,
    },
};
use vfs::FileId;

use crate::{
    compilation_plan,
    db::{CompilationDiagnostic, PreprocDb},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCompilationJob {
    pub profile_id: u32,
    pub roots: Vec<ProfileCompilationRoot>,
    pub buffers: Vec<ProfileCompilationBuffer>,
    pub top_modules: Vec<String>,
    pub include_dirs: Vec<String>,
    pub predefines: Vec<String>,
    pub diagnostics: ProfileDiagnosticsOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCompilationRoot {
    pub file_id: u32,
    pub kind: ProfileRootKind,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCompilationBuffer {
    pub file_id: u32,
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileRootKind {
    SystemVerilog,
    LibraryMap,
}

impl From<compilation_plan::CompilationRootKind> for ProfileRootKind {
    fn from(kind: compilation_plan::CompilationRootKind) -> Self {
        match kind {
            compilation_plan::CompilationRootKind::SystemVerilog => Self::SystemVerilog,
            compilation_plan::CompilationRootKind::LibraryMap => Self::LibraryMap,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDiagnosticsOptions {
    pub parse: bool,
    pub semantic: bool,
    pub warnings: Option<Vec<String>>,
    pub rules: Vec<ProfileDiagnosticRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDiagnosticRule {
    pub selector: ProfileDiagnosticSelector,
    pub severity: ProfileDiagnosticRuleSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileDiagnosticSelector {
    Code { subsystem: u16, code: u16 },
    Option(String),
    Group(String),
    Source(ProfileDiagnosticSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileDiagnosticRuleSeverity {
    Ignore,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileDiagnosticSource {
    Parse,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCompilationOutput {
    pub diagnostics: Vec<ProfileCompilationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileCompilationDiagnostic {
    pub file_id: u32,
    pub source: ProfileDiagnosticSource,
    pub diagnostic: SyntaxDiagnosticWire,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticWire {
    pub code: u16,
    pub subsystem: u16,
    pub severity: DiagnosticSeverityWire,
    pub message: String,
    pub args: Vec<String>,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub primary_range: Option<Range<usize>>,
    pub location: Option<usize>,
    pub buffer_id: Option<u32>,
    pub file_name: Option<String>,
    pub ranges: Vec<SyntaxDiagnosticRangeWire>,
    pub expansion_locations: Vec<SyntaxDiagnosticExpansionWire>,
    pub include_stack: Vec<SyntaxDiagnosticLocationWire>,
    pub diagnostic_id: u32,
    pub parent_diagnostic_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticLocationWire {
    pub offset: usize,
    pub buffer_id: u32,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticRangeWire {
    pub start: usize,
    pub end: usize,
    pub start_buffer_id: u32,
    pub end_buffer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntaxDiagnosticExpansionWire {
    pub location: Option<SyntaxDiagnosticLocationWire>,
    pub original_location: Option<SyntaxDiagnosticLocationWire>,
    pub macro_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverityWire {
    Ignored,
    Note,
    Warning,
    Error,
    Fatal,
}

pub fn build_profile_compilation_job(
    db: &dyn PreprocDb,
    profile_id: CompilationProfileId,
) -> ProfileCompilationJob {
    let plan = db.compilation_plan_for_profile(Some(profile_id));
    let context = db.compilation_context(Some(profile_id));
    let config = db.diagnostics_config();
    let buffers = compilation_plan::compilation_source_buffers_for_plan(db, &plan)
        .into_iter()
        .map(|buffer| ProfileCompilationBuffer {
            file_id: buffer.file_id.index(),
            path: buffer.path,
            text: buffer.text,
        })
        .collect();
    let roots = plan
        .roots
        .iter()
        .copied()
        .map(|root| {
            let path = compilation_plan::source_buffer_path(db, root.file_id).to_string();
            let name = db
                .file_path(root.file_id)
                .map(|path| path.to_string())
                .unwrap_or_else(|| "source".to_owned());
            ProfileCompilationRoot {
                file_id: root.file_id.index(),
                kind: ProfileRootKind::from(root.kind),
                name,
                path,
            }
        })
        .collect();
    ProfileCompilationJob {
        profile_id: profile_id.0,
        roots,
        buffers,
        top_modules: context.top_modules.to_vec(),
        include_dirs: context.include_dirs.iter().map(ToString::to_string).collect(),
        predefines: context.predefines.to_vec(),
        diagnostics: diagnostics_options(&config),
    }
}

pub fn run_profile_compilation(job: ProfileCompilationJob) -> ProfileCompilationOutput {
    let mut compilation = Compilation::new_with_top_modules(&job.top_modules);
    compilation.register_source_buffers(
        &job.buffers
            .iter()
            .map(|buffer| SyntaxTreeBuffer { path: buffer.path.clone(), text: buffer.text.clone() })
            .collect::<Vec<_>>(),
    );
    let path_file_ids = job
        .buffers
        .iter()
        .map(|buffer| (buffer.path.as_str(), buffer.file_id))
        .collect::<FxHashMap<_, _>>();
    let mut buffer_file_ids = FxHashMap::default();
    for root in &job.roots {
        let options = match root.kind {
            ProfileRootKind::SystemVerilog => SyntaxTreeOptions {
                predefines: job.predefines.clone(),
                include_paths: job.include_dirs.clone(),
                include_buffers: Vec::new(),
                ..SyntaxTreeOptions::default()
            },
            ProfileRootKind::LibraryMap => SyntaxTreeOptions::default(),
        };
        let tree = match root.kind {
            ProfileRootKind::SystemVerilog => {
                compilation.parse_syntax_tree_from_buffer(&root.name, &root.path, &options)
            }
            ProfileRootKind::LibraryMap => compilation
                .parse_library_map_syntax_tree_from_buffer(&root.name, &root.path, &options),
        };
        let buffers = tree.buffer_ids();
        buffer_file_ids.insert(buffers.root_buffer_id, root.file_id);
        for source in buffers.source_buffers {
            if let Some(file_id) = path_file_ids.get(source.path.as_str()) {
                buffer_file_ids.insert(source.buffer_id, *file_id);
            }
        }
    }

    let warning_options = match &job.diagnostics.warnings {
        Some(options) if options.is_empty() => vec!["none".to_owned()],
        Some(options) => options.clone(),
        None => Vec::new(),
    };
    let mut diagnostics = Vec::new();
    if job.diagnostics.parse {
        collect_diagnostics(
            &job.diagnostics,
            ProfileDiagnosticSource::Parse,
            compilation.parse_diagnostics_with_options(&warning_options),
            &buffer_file_ids,
            &mut diagnostics,
        );
    }
    if job.diagnostics.semantic {
        collect_diagnostics(
            &job.diagnostics,
            ProfileDiagnosticSource::Semantic,
            compilation.semantic_diagnostics_with_options(&warning_options),
            &buffer_file_ids,
            &mut diagnostics,
        );
    }
    ProfileCompilationOutput { diagnostics }
}

impl ProfileCompilationOutput {
    pub fn into_diagnostics(self) -> Vec<CompilationDiagnostic> {
        self.diagnostics
            .into_iter()
            .map(|diagnostic| CompilationDiagnostic {
                file_id: FileId::from_raw(diagnostic.file_id),
                source: match diagnostic.source {
                    ProfileDiagnosticSource::Parse => DiagnosticSource::Parse,
                    ProfileDiagnosticSource::Semantic => DiagnosticSource::Semantic,
                },
                diagnostic: diagnostic.diagnostic.into(),
            })
            .collect()
    }
}

fn diagnostics_options(config: &DiagnosticsConfig) -> ProfileDiagnosticsOptions {
    ProfileDiagnosticsOptions {
        parse: config.enabled && config.parse.enabled,
        semantic: config.enabled && config.semantic.enabled,
        warnings: config.slang.warnings.clone(),
        rules: config
            .slang
            .rules
            .iter()
            .map(|rule| ProfileDiagnosticRule {
                selector: match &rule.selector {
                    DiagnosticSelector::Code { subsystem, code } => {
                        ProfileDiagnosticSelector::Code { subsystem: *subsystem, code: *code }
                    }
                    DiagnosticSelector::Option(option) => {
                        ProfileDiagnosticSelector::Option(option.clone())
                    }
                    DiagnosticSelector::Group(group) => {
                        ProfileDiagnosticSelector::Group(group.clone())
                    }
                    DiagnosticSelector::Source(source) => {
                        ProfileDiagnosticSelector::Source(match source {
                            DiagnosticSource::Parse => ProfileDiagnosticSource::Parse,
                            DiagnosticSource::Semantic => ProfileDiagnosticSource::Semantic,
                        })
                    }
                },
                severity: match rule.severity {
                    DiagnosticRuleSeverity::Ignore => ProfileDiagnosticRuleSeverity::Ignore,
                    DiagnosticRuleSeverity::Info => ProfileDiagnosticRuleSeverity::Info,
                    DiagnosticRuleSeverity::Warning => ProfileDiagnosticRuleSeverity::Warning,
                    DiagnosticRuleSeverity::Error => ProfileDiagnosticRuleSeverity::Error,
                    DiagnosticRuleSeverity::Fatal => ProfileDiagnosticRuleSeverity::Fatal,
                },
            })
            .collect(),
    }
}

fn collect_diagnostics(
    options: &ProfileDiagnosticsOptions,
    source: ProfileDiagnosticSource,
    raw: Vec<SyntaxDiagnostic>,
    buffer_file_ids: &FxHashMap<u32, u32>,
    diagnostics: &mut Vec<ProfileCompilationDiagnostic>,
) {
    diagnostics.extend(raw.into_iter().filter_map(|diagnostic| {
        let file_id =
            diagnostic.buffer_id.and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied())?;
        let diagnostic = apply_rules(options, source, diagnostic)?;
        Some(ProfileCompilationDiagnostic { file_id, source, diagnostic: diagnostic.into() })
    }));
}

fn apply_rules(
    options: &ProfileDiagnosticsOptions,
    source: ProfileDiagnosticSource,
    mut diagnostic: SyntaxDiagnostic,
) -> Option<SyntaxDiagnostic> {
    for rule in &options.rules {
        let matches = match &rule.selector {
            ProfileDiagnosticSelector::Code { subsystem, code } => {
                diagnostic.subsystem == *subsystem && diagnostic.code == *code
            }
            ProfileDiagnosticSelector::Option(option) => {
                diagnostic.option_name.as_deref() == Some(option)
            }
            ProfileDiagnosticSelector::Group(group) => {
                diagnostic.groups.iter().any(|candidate| candidate == group)
            }
            ProfileDiagnosticSelector::Source(rule_source) => source == *rule_source,
        };
        if !matches {
            continue;
        }
        diagnostic.severity = match rule.severity {
            ProfileDiagnosticRuleSeverity::Ignore => return None,
            ProfileDiagnosticRuleSeverity::Info => DiagnosticSeverity::Note,
            ProfileDiagnosticRuleSeverity::Warning => DiagnosticSeverity::Warning,
            ProfileDiagnosticRuleSeverity::Error => DiagnosticSeverity::Error,
            ProfileDiagnosticRuleSeverity::Fatal => DiagnosticSeverity::Fatal,
        };
    }
    (diagnostic.severity != DiagnosticSeverity::Ignored).then_some(diagnostic)
}

impl From<SyntaxDiagnostic> for SyntaxDiagnosticWire {
    fn from(diagnostic: SyntaxDiagnostic) -> Self {
        Self {
            code: diagnostic.code,
            subsystem: diagnostic.subsystem,
            severity: diagnostic.severity.into(),
            message: diagnostic.message,
            args: diagnostic.args,
            name: diagnostic.name,
            option_name: diagnostic.option_name,
            groups: diagnostic.groups,
            primary_range: diagnostic.primary_range,
            location: diagnostic.location,
            buffer_id: diagnostic.buffer_id,
            file_name: diagnostic.file_name,
            ranges: diagnostic.ranges.into_iter().map(Into::into).collect(),
            expansion_locations: diagnostic
                .expansion_locations
                .into_iter()
                .map(Into::into)
                .collect(),
            include_stack: diagnostic.include_stack.into_iter().map(Into::into).collect(),
            diagnostic_id: diagnostic.diagnostic_id,
            parent_diagnostic_id: diagnostic.parent_diagnostic_id,
        }
    }
}

impl From<SyntaxDiagnosticWire> for SyntaxDiagnostic {
    fn from(diagnostic: SyntaxDiagnosticWire) -> Self {
        Self {
            code: diagnostic.code,
            subsystem: diagnostic.subsystem,
            severity: diagnostic.severity.into(),
            message: diagnostic.message,
            args: diagnostic.args,
            name: diagnostic.name,
            option_name: diagnostic.option_name,
            groups: diagnostic.groups,
            primary_range: diagnostic.primary_range,
            location: diagnostic.location,
            buffer_id: diagnostic.buffer_id,
            file_name: diagnostic.file_name,
            ranges: diagnostic.ranges.into_iter().map(Into::into).collect(),
            expansion_locations: diagnostic
                .expansion_locations
                .into_iter()
                .map(Into::into)
                .collect(),
            include_stack: diagnostic.include_stack.into_iter().map(Into::into).collect(),
            diagnostic_id: diagnostic.diagnostic_id,
            parent_diagnostic_id: diagnostic.parent_diagnostic_id,
        }
    }
}

impl From<DiagnosticSeverity> for DiagnosticSeverityWire {
    fn from(severity: DiagnosticSeverity) -> Self {
        match severity {
            DiagnosticSeverity::Ignored => Self::Ignored,
            DiagnosticSeverity::Note => Self::Note,
            DiagnosticSeverity::Warning => Self::Warning,
            DiagnosticSeverity::Error => Self::Error,
            DiagnosticSeverity::Fatal => Self::Fatal,
        }
    }
}

impl From<DiagnosticSeverityWire> for DiagnosticSeverity {
    fn from(severity: DiagnosticSeverityWire) -> Self {
        match severity {
            DiagnosticSeverityWire::Ignored => Self::Ignored,
            DiagnosticSeverityWire::Note => Self::Note,
            DiagnosticSeverityWire::Warning => Self::Warning,
            DiagnosticSeverityWire::Error => Self::Error,
            DiagnosticSeverityWire::Fatal => Self::Fatal,
        }
    }
}

impl From<SyntaxDiagnosticLocation> for SyntaxDiagnosticLocationWire {
    fn from(location: SyntaxDiagnosticLocation) -> Self {
        Self {
            offset: location.offset,
            buffer_id: location.buffer_id,
            file_name: location.file_name,
        }
    }
}

impl From<SyntaxDiagnosticLocationWire> for SyntaxDiagnosticLocation {
    fn from(location: SyntaxDiagnosticLocationWire) -> Self {
        Self {
            offset: location.offset,
            buffer_id: location.buffer_id,
            file_name: location.file_name,
        }
    }
}

impl From<SyntaxDiagnosticRange> for SyntaxDiagnosticRangeWire {
    fn from(range: SyntaxDiagnosticRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
            start_buffer_id: range.start_buffer_id,
            end_buffer_id: range.end_buffer_id,
        }
    }
}

impl From<SyntaxDiagnosticRangeWire> for SyntaxDiagnosticRange {
    fn from(range: SyntaxDiagnosticRangeWire) -> Self {
        Self {
            start: range.start,
            end: range.end,
            start_buffer_id: range.start_buffer_id,
            end_buffer_id: range.end_buffer_id,
        }
    }
}

impl From<SyntaxDiagnosticExpansion> for SyntaxDiagnosticExpansionWire {
    fn from(expansion: SyntaxDiagnosticExpansion) -> Self {
        Self {
            location: expansion.location.map(Into::into),
            original_location: expansion.original_location.map(Into::into),
            macro_name: expansion.macro_name,
        }
    }
}

impl From<SyntaxDiagnosticExpansionWire> for SyntaxDiagnosticExpansion {
    fn from(expansion: SyntaxDiagnosticExpansionWire) -> Self {
        Self {
            location: expansion.location.map(Into::into),
            original_location: expansion.original_location.map(Into::into),
            macro_name: expansion.macro_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(text: &str) -> ProfileCompilationJob {
        ProfileCompilationJob {
            profile_id: 0,
            roots: vec![ProfileCompilationRoot {
                file_id: 0,
                kind: ProfileRootKind::SystemVerilog,
                name: "/rtl/top.sv".to_owned(),
                path: "/rtl/top.sv".to_owned(),
            }],
            buffers: vec![ProfileCompilationBuffer {
                file_id: 0,
                path: "/rtl/top.sv".to_owned(),
                text: text.to_owned(),
            }],
            top_modules: Vec::new(),
            include_dirs: vec!["/rtl".to_owned()],
            predefines: Vec::new(),
            diagnostics: ProfileDiagnosticsOptions {
                parse: true,
                semantic: true,
                warnings: Some(Vec::new()),
                rules: Vec::new(),
            },
        }
    }

    #[test]
    fn job_round_trips_through_json() {
        let job = job("module top; endmodule\n");
        let encoded = serde_json::to_vec(&job).unwrap();
        let decoded: ProfileCompilationJob = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, job);
    }

    #[test]
    fn parse_diagnostics_are_attributed_to_the_root() {
        let output = run_profile_compilation(job("module top(;\nendmodule\n"));
        assert!(output.diagnostics.iter().any(|diagnostic| diagnostic.file_id == 0), "{output:?}");
    }

    #[test]
    fn source_rule_filters_worker_diagnostics() {
        let mut job = job("module top(;\nendmodule\n");
        job.diagnostics.semantic = false;
        job.diagnostics.rules.push(ProfileDiagnosticRule {
            selector: ProfileDiagnosticSelector::Source(ProfileDiagnosticSource::Parse),
            severity: ProfileDiagnosticRuleSeverity::Ignore,
        });
        assert!(run_profile_compilation(job).diagnostics.is_empty());
    }

    #[test]
    fn included_buffer_diagnostics_keep_their_file_identity() {
        let mut job = job("`include \"defs.svh\"\nmodule top; endmodule\n");
        job.buffers.push(ProfileCompilationBuffer {
            file_id: 1,
            path: "/rtl/defs.svh".to_owned(),
            text: "module broken(;\nendmodule\n".to_owned(),
        });
        let output = run_profile_compilation(job);
        assert!(output.diagnostics.iter().any(|diagnostic| diagnostic.file_id == 1), "{output:?}");
    }

    #[test]
    fn library_map_roots_use_the_profile_session() {
        let mut job = job("");
        job.roots[0].kind = ProfileRootKind::LibraryMap;
        job.buffers[0].text = "library work \"/rtl/*.sv\";\n".to_owned();
        let output = run_profile_compilation(job);
        assert!(output.diagnostics.is_empty(), "{output:?}");
    }
}
