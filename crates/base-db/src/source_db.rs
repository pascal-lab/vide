use rustc_hash::FxHashSet;
use syntax::{Compilation, SyntaxDiagnostic, SyntaxTree};
use triomphe::Arc;
use utils::line_index::TextSize;
use vfs::{FileId, anchored_path::AnchoredPath};

use crate::{
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
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

    fn parse_src(&self, file_id: FileId) -> SyntaxTree;
    fn expected_identifier_offsets(&self, file_id: FileId) -> Arc<Vec<TextSize>>;
    fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;

    #[salsa::input]
    fn files(&self) -> Box<FxHashSet<FileId>>;

    #[salsa::input]
    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig>;
}

fn parse_src(db: &dyn SourceDb, file_id: FileId) -> SyntaxTree {
    let text = db.file_text(file_id);
    // TODO: use meaningful path
    let tree = SyntaxTree::from_text(&text, "", "");
    if tree.diagnostics().is_empty() {
        return tree;
    }

    // Library maps are a separate Verilog input grammar. Only switch modes when the
    // ordinary source parser fails and the library-map parser accepts the file
    // cleanly.
    let library_map = SyntaxTree::from_library_map_text(&text, "", "");
    if library_map.diagnostics().is_empty() {
        return library_map;
    }

    tree
}

fn expected_identifier_offsets(db: &dyn SourceDb, file_id: FileId) -> Arc<Vec<TextSize>> {
    let tree = db.parse_src(file_id);
    let mut compilation = Compilation::new();
    compilation.add_syntax_tree(tree.clone());
    let config = db.diagnostics_config();
    let mut out: Vec<TextSize> = compilation
        .parse_diag_offsets_by_name("ExpectedIdentifier", &config.slang.warnings)
        .into_iter()
        .filter_map(|offset| u32::try_from(offset).ok().map(TextSize::from))
        .collect();
    out.sort();
    out.dedup();
    Arc::new(out)
}

fn parse_diagnostics(db: &dyn SourceDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.parse.enabled {
        return Arc::from(Vec::<SyntaxDiagnostic>::new());
    }

    let tree = db.parse_src(file_id);
    let diags = tree
        .diagnostics_with_options(&config.slang.warnings)
        .into_iter()
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

    fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    fn source_root_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Arc<[(FileId, SyntaxDiagnostic)]>;
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
    if !config.enabled || !config.semantic.enabled {
        return Arc::from(Vec::<(FileId, SyntaxDiagnostic)>::new());
    }

    let source_root = db.source_root(source_root_id);
    let mut compilation = Compilation::new();
    let mut buffer_file_ids = rustc_hash::FxHashMap::default();

    for file_id in source_root.iter() {
        let tree = db.parse_src(file_id);
        buffer_file_ids.insert(tree.buffer_id(), file_id);
        compilation.add_syntax_tree(tree);
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
