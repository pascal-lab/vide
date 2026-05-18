use rustc_hash::{FxHashMap, FxHashSet};
use syntax::{
    Compilation, ParserExpectedSyntax, SyntaxDiagnostic, SyntaxTree, SyntaxTreeBuffer,
    SyntaxTreeBufferIds,
};
use triomphe::Arc;
use utils::line_index::TextSize;
use vfs::{FileId, VfsPath, anchored_path::AnchoredPath};

use crate::{
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
    project::{CompilationProfileId, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};

pub trait FileLoader {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

// Source code, syntax tree and project model.
// Everything else is derived from these queries.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn file_kind(&self, file_id: FileId) -> SourceFileKind;

    #[salsa::input]
    fn file_path(&self, file_id: FileId) -> Option<utils::paths::AbsPathBuf>;

    fn parse_src(&self, file_id: FileId) -> SyntaxTree;

    #[salsa::input]
    fn files(&self) -> Box<FxHashSet<FileId>>;

    #[salsa::input]
    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig>;

    #[salsa::input]
    fn project_config(&self) -> Arc<ProjectConfig>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFileKind {
    #[default]
    SystemVerilog,
    IncludeHeader,
    LibraryMap,
    ProjectManifest,
}

impl SourceFileKind {
    pub fn from_path(path: &VfsPath) -> Self {
        match path.name_and_extension() {
            Some(("vizsla_config", Some(ext))) if ext.eq_ignore_ascii_case("toml") => {
                Self::ProjectManifest
            }
            Some((_, Some(ext))) if ext.eq_ignore_ascii_case("map") => Self::LibraryMap,
            Some((_, Some(ext)))
                if ["vh", "svh", "svi"].iter().any(|header| ext.eq_ignore_ascii_case(header)) =>
            {
                Self::IncludeHeader
            }
            _ => Self::SystemVerilog,
        }
    }

    fn is_semantic_compilation_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }

    fn is_slang_parse_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }
}

fn parse_src(db: &dyn SourceDb, file_id: FileId) -> SyntaxTree {
    let text = db.file_text(file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            SyntaxTree::from_text(&text, "", "")
        }
        SourceFileKind::LibraryMap => SyntaxTree::from_library_map_text(&text, "", ""),
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    }
}

struct SourceFileIdentity {
    name: String,
    path: String,
}

fn source_file_identity(db: &dyn SourceDb, file_id: FileId) -> SourceFileIdentity {
    let path = db.file_path(file_id).map(|path| path.to_string()).unwrap_or_default();
    let name = if path.is_empty() { "source".to_owned() } else { path.clone() };
    SourceFileIdentity { name, path }
}

fn normalized_path_key(path: &str) -> String {
    path.replace('\\', "/")
}

fn insert_buffer_file_ids(
    buffer_file_ids: &mut FxHashMap<u32, FileId>,
    path_file_ids: &FxHashMap<String, FileId>,
    buffers: SyntaxTreeBufferIds,
    root_file_id: FileId,
) {
    buffer_file_ids.insert(buffers.root_buffer_id, root_file_id);
    for buffer in buffers.source_buffers {
        if let Some(file_id) = path_file_ids.get(&normalized_path_key(&buffer.path)) {
            buffer_file_ids.insert(buffer.buffer_id, *file_id);
        }
    }
}

fn syntax_tree_options_for_file(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let project_config = db.project_config();
    let profile_id = db.file_compilation_profile(file_id);
    let preprocess = project_config.preprocess_for_profile(profile_id);
    let include_paths = preprocess.include_dir_strings();
    let include_buffers = db.include_buffers_for_profile(profile_id).as_ref().clone();
    syntax::SyntaxTreeOptions { predefines: preprocess.predefines, include_paths, include_buffers }
}

fn parse_src_for_compilation(db: &dyn SourceRootDb, file_id: FileId) -> SyntaxTree {
    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            SyntaxTree::from_text_with_options(&text, &identity.name, &identity.path, &options)
        }
        SourceFileKind::LibraryMap => {
            SyntaxTree::from_library_map_text(&text, &identity.name, &identity.path)
        }
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    }
}

fn in_memory_include_buffers(
    db: &dyn SourceRootDb,
    include_dirs: &[utils::paths::AbsPathBuf],
) -> Vec<SyntaxTreeBuffer> {
    let mut seen = FxHashSet::default();
    let mut buffers = Vec::new();

    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }

        if !matches!(db.file_kind(file_id), SourceFileKind::IncludeHeader) {
            continue;
        }

        let Some(path) = db.file_path(file_id) else {
            continue;
        };

        let in_include_path = include_dirs.iter().any(|include_dir| path.starts_with(include_dir));
        if !in_include_path {
            continue;
        }

        let path = path.to_string();
        if !seen.insert(path.clone()) {
            continue;
        }

        buffers.push(SyntaxTreeBuffer { path, text: db.file_text(file_id).to_string() });
    }

    buffers
}

fn parser_expected_syntax(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> Arc<[ParserExpectedSyntax]> {
    if matches!(db.file_kind(file_id), SourceFileKind::ProjectManifest) {
        return Arc::from(Vec::<ParserExpectedSyntax>::new());
    }

    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let offset = usize::from(offset);
    let expected = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            SyntaxTree::expected_syntax_at_offset_with_options(
                &text,
                &identity.name,
                &identity.path,
                offset,
                &options,
            )
        }
        SourceFileKind::LibraryMap => SyntaxTree::library_map_expected_syntax_at_offset(
            &text,
            &identity.name,
            &identity.path,
            offset,
        ),
        SourceFileKind::ProjectManifest => Vec::new(),
    };
    Arc::from(expected)
}

fn parse_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.parse.enabled || !db.file_kind(file_id).is_slang_parse_unit() {
        return Arc::from(Vec::<SyntaxDiagnostic>::new());
    }

    let tree = db.parse_src_for_compilation(file_id);
    let root_buffer_id = tree.buffer_id();
    let diags = tree
        .diagnostics_with_options(&config.slang.warnings)
        .into_iter()
        .filter(|diag| diag.buffer_id.is_none_or(|buffer_id| buffer_id == root_buffer_id))
        .filter_map(|diag| config.apply_rules(DiagnosticSource::Parse, diag))
        .collect::<Vec<_>>();
    Arc::from(diags)
}

// Don't expose source roots to HIR, so extract them in a separate DB.
#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;

    fn file_compilation_profile(&self, file_id: FileId) -> Option<CompilationProfileId>;
    fn file_is_project_ignored(&self, file_id: FileId) -> bool;
    fn include_buffers_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>>;
    fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree;
    fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]>;
    fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    fn source_root_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Arc<[(FileId, SyntaxDiagnostic)]>;
}

fn file_compilation_profile(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Option<CompilationProfileId> {
    let source_root_id = db.source_root_id(file_id);
    let project_config = db.project_config();
    let profile_id = project_config.profile_for_root(source_root_id);
    if profile_id.is_none() && !db.source_root(source_root_id).is_ignored() {
        tracing::debug!(
            ?file_id,
            ?source_root_id,
            root_profile_count = project_config.root_profile_count(),
            "file has no compilation profile",
        );
    }
    profile_id
}

fn file_is_project_ignored(db: &dyn SourceRootDb, file_id: FileId) -> bool {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).is_ignored()
}

fn include_buffers_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<Vec<SyntaxTreeBuffer>> {
    let project_config = db.project_config();
    let preprocess = project_config.preprocess_for_profile(profile_id);
    Arc::new(in_memory_include_buffers(db, &preprocess.include_dirs))
}

fn semantic_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    Arc::from(
        db.source_root_semantic_diagnostics(file_id)
            .iter()
            .filter_map(|(diag_file_id, diag)| (*diag_file_id == file_id).then_some(diag.clone()))
            .collect::<Vec<_>>(),
    )
}

fn source_root_semantic_diagnostics(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<[(FileId, SyntaxDiagnostic)]> {
    let source_root_id = db.source_root_id(file_id);
    let config = db.diagnostics_config();
    if !config.enabled || !config.semantic.enabled || db.file_is_project_ignored(file_id) {
        return Arc::from(Vec::<(FileId, SyntaxDiagnostic)>::new());
    }

    let project_config = db.project_config();
    let profile = project_config.profile_for_root(source_root_id);
    let profile = profile.and_then(|profile_id| project_config.profile(profile_id));
    let compilation_roots =
        profile.map(|profile| profile.source_roots.clone()).unwrap_or_else(|| vec![source_root_id]);
    let top_modules = profile.map(|profile| profile.top_modules.clone()).unwrap_or_default();
    let mut compilation = Compilation::new_with_top_modules(&top_modules);
    let mut buffer_file_ids = FxHashMap::default();
    let mut path_file_ids = FxHashMap::default();
    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        if let Some(path) = db.file_path(file_id) {
            path_file_ids.insert(normalized_path_key(&path.to_string()), file_id);
        }
    }
    let mut visited_files = FxHashSet::default();

    for root_id in compilation_roots {
        let source_root = db.source_root(root_id);
        for file_id in source_root.iter() {
            if !visited_files.insert(file_id) {
                continue;
            }
            if db.file_is_project_ignored(file_id) {
                continue;
            }
            if !db.file_kind(file_id).is_semantic_compilation_unit() {
                continue;
            }
            let text = db.file_text(file_id);
            let identity = source_file_identity(db, file_id);
            let buffer_ids = match db.file_kind(file_id) {
                SourceFileKind::SystemVerilog => {
                    let options = syntax_tree_options_for_file(db, file_id);
                    compilation.add_syntax_tree_from_text(
                        &text,
                        &identity.name,
                        &identity.path,
                        &options,
                    )
                }
                SourceFileKind::LibraryMap => compilation.add_library_map_syntax_tree_from_text(
                    &text,
                    &identity.name,
                    &identity.path,
                ),
                SourceFileKind::IncludeHeader | SourceFileKind::ProjectManifest => continue,
            };
            insert_buffer_file_ids(&mut buffer_file_ids, &path_file_ids, buffer_ids, file_id);
        }
    }

    let diagnostics = compilation
        .semantic_diagnostics_with_options(&config.slang.warnings)
        .into_iter()
        .filter_map(|diag| {
            let diag_file_id =
                diag.buffer_id.and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied())?;
            let diag = config.apply_rules(DiagnosticSource::Semantic, diag)?;
            Some((diag_file_id, diag))
        })
        .collect::<Vec<_>>();

    Arc::from(diagnostics)
}

#[cfg(test)]
mod tests {
    use vfs::VfsPath;

    use super::*;

    #[test]
    fn include_headers_are_not_standalone_parse_diagnostic_units() {
        let kind =
            SourceFileKind::from_path(&VfsPath::new_virtual_path("/include/defs.svh".into()));

        assert_eq!(kind, SourceFileKind::IncludeHeader);
        assert!(!kind.is_slang_parse_unit());
    }

    #[test]
    fn systemverilog_sources_remain_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/rtl/top.sv".into()));

        assert_eq!(kind, SourceFileKind::SystemVerilog);
        assert!(kind.is_slang_parse_unit());
    }
}
