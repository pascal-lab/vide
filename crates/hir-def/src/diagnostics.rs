//! File-level aggregation of lowering diagnostics.
//!
//! Every lowering pass stores recovery diagnostics beside its position-free
//! result; this module projects their `SourceAstId` anchors and flattens them
//! for a whole file without exposing owner-specific lowering to the IDE.
//!
//! A lowerer reports a diagnostic without a root-buffer range (`range: None`)
//! when the offending syntax has no stable position in the file's display
//! coordinates — e.g. syntax inside an included buffer. Those diagnostics
//! cannot be published to an editor as-is, so the aggregate resolves a
//! conservative display range for them, in order:
//!
//! 1. the explicit range, when the lowerer attached one;
//! 2. the projected root-buffer range of the source identity;
//! 3. a zero-width marker at the start of the owning container;
//! 4. the file start, as a last resort.

use la_arena::Arena;
use syntax::SyntaxTree;
use triomphe::Arc;
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};

use crate::{
    ast_id_map::SyntaxFileId, body::BodyItem, db::HirDefDb, has_source::HasSource, owner::OwnerId,
    proc::Proc, source_map::LoweringDiagnostic, source_projection::SourceProjection,
};

#[salsa::tracked(returns(clone))]
pub(crate) fn file_lowering_diagnostics(
    db: &dyn HirDefDb,
    file: SyntaxFileId,
) -> Arc<[LoweringDiagnostic]> {
    let file_id = file.hir_file(db);
    let tree = db.parse(file_id);
    let file_owner = db.owner_table(file_id).file_owner().expect("file owner");
    let lowered_file = db.body_with_source_map(file_owner);
    let file = lowered_file.data_ref();

    let mut diagnostics = Vec::new();
    let projection = db.source_projection(file_id);
    // File-level owners have no enclosing container; the whole file is the
    // search scope for range-less diagnostics.
    collect(lowered_file.raw_diagnostics(), None, &tree, &projection, &mut diagnostics);

    for owner in file.subroutine_owners() {
        collect_subroutine(db, owner, &tree, &projection, &mut diagnostics);
    }

    for owner in file.module_owners() {
        collect_module(db, owner, &tree, &projection, &mut diagnostics);
    }

    collect_proc_bodies(db, &file.procs, &tree, &projection, &mut diagnostics);
    Arc::from(diagnostics)
}

fn collect_module(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);

    let module = lowered.data_ref();
    let src_map = lowered.source_map();
    for owner in module.subroutine_owners() {
        collect_subroutine(db, owner, tree, projection, diagnostics);
    }

    for region_id in src_map.generate_region_srcs.iter().map(|(id, _)| id) {
        let region = module.get(region_id);
        for item in &region.items {
            if let BodyItem::GenerateBlockOwner(block_id) = item {
                collect_generate_block(db, *block_id, tree, projection, diagnostics);
            }
        }
    }
    collect_proc_bodies(db, &module.procs, tree, projection, diagnostics);
}

fn collect_generate_block(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);

    let block = lowered.data_ref();
    for owner in block.subroutine_owners() {
        collect_subroutine(db, owner, tree, projection, diagnostics);
    }
    for item in &block.items {
        if let BodyItem::GenerateBlockOwner(nested) = item {
            collect_generate_block(db, *nested, tree, projection, diagnostics);
        }
    }
    collect_proc_bodies(db, &block.procs, tree, projection, diagnostics);
}

fn collect_subroutine(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    let lowered = db.body_with_source_map(owner);
    let owner_range = owner.source(db).map(|source| source.value.full_range());
    collect(lowered.raw_diagnostics(), owner_range, tree, projection, diagnostics);
}

fn collect_proc_bodies(
    db: &dyn HirDefDb,
    procs: &Arena<Proc>,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    diagnostics: &mut Vec<LoweringDiagnostic>,
) {
    for (_, proc) in procs.iter() {
        let body = db.body_with_source_map(proc.owner);
        let owner_range = proc.owner.source(db).map(|source| source.value.full_range());
        collect(body.raw_diagnostics(), owner_range, tree, projection, diagnostics);
    }
}

fn collect(
    diagnostics: &[LoweringDiagnostic],
    owner_range: Option<TextRange>,
    tree: &SyntaxTree,
    projection: &SourceProjection,
    out: &mut Vec<LoweringDiagnostic>,
) {
    for diagnostic in diagnostics {
        let mut diagnostic = diagnostic.clone();
        if diagnostic.range.is_none() {
            diagnostic.range = diagnostic
                .source
                .and_then(|source| projection.origin(source))
                .and_then(|origin| origin.full_range())
                .or_else(|| Some(resolve_range(tree, owner_range, &diagnostic)));
        }
        out.push(diagnostic);
    }
}

/// Resolves a conservative display range without guessing from unrelated
/// syntax of the same kind.
fn resolve_range(
    _tree: &SyntaxTree,
    owner_range: Option<TextRange>,
    diagnostic: &LoweringDiagnostic,
) -> TextRange {
    diagnostic
        .range
        .or_else(|| owner_range.map(|range| TextRange::empty(range.start())))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(0)))
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{FileLoader, SourceDb, SourceFileKind, SourceRootDb},
        source_root::{SourceRoot, SourceRootId},
    };
    use preproc_expand::{db::PreprocDb, file::HirFileId};
    use rustc_hash::FxHashSet;
    use syntax::{SyntaxKind, SyntaxTree};
    use triomphe::Arc;
    use utils::{
        paths::{AbsPathBuf, Utf8PathBuf},
        text_edit::{TextRange, TextSize},
    };
    use vfs::{AnchoredPath, FileId, FileSet, VfsPath};

    use crate::{
        db::HirDefDb,
        source_map::{LoweringDiagnostic, LoweringDiagnosticKind},
    };

    const TOP: FileId = FileId::from_raw(0);
    const HEADER: FileId = FileId::from_raw(1);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::db]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    #[salsa::db]
    impl salsa::Database for TestDb {}

    #[salsa::db]
    impl SourceDb for TestDb {}

    #[salsa::db]
    impl SourceRootDb for TestDb {}

    #[salsa::db]
    impl PreprocDb for TestDb {}

    #[salsa::db]
    impl HirDefDb for TestDb {}

    impl std::ops::Deref for TestDb {
        type Target = dyn HirDefDb;

        fn deref(&self) -> &Self::Target {
            self
        }
    }

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_files(root_text: &str, header_text: Option<&str>) -> TestDb {
        let root_path = abs_path("rtl");
        let top_path = root_path.join("top.sv");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let mut source_files = vec![TOP];
        if header_text.is_some() {
            let header_path = root_path.join("defs.vh");
            file_set.insert(HEADER, VfsPath::from(header_path));
            source_files.push(HEADER);
        }
        let root = SourceRoot::new_local_with_source_files(file_set, source_files);
        let mut files = FxHashSet::default();
        files.insert(TOP);
        if header_text.is_some() {
            files.insert(HEADER);
        }

        let preprocess = PreprocessConfig {
            include_dirs: vec![root_path.clone()],
            ..PreprocessConfig::default()
        };
        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: preprocess.clone(),
            }],
        );

        let mut db = TestDb::default();
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_path_with_durability(TOP, Some(top_path), Durability::LOW);
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(TOP, Arc::from(root_text), Durability::LOW);
        if let Some(header_text) = header_text {
            let header_path = root_path.join("defs.vh");
            db.set_source_root_id_with_durability(HEADER, ROOT, Durability::LOW);
            db.set_file_path_with_durability(HEADER, Some(header_path), Durability::LOW);
            db.set_file_kind_with_durability(
                HEADER,
                SourceFileKind::IncludeHeader,
                Durability::LOW,
            );
            db.set_file_text_with_durability(HEADER, Arc::from(header_text), Durability::LOW);
        }
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn range_of(text: &str, needle: &str) -> TextRange {
        range_of_nth(text, needle, 0)
    }

    fn range_of_nth(text: &str, needle: &str, nth: usize) -> TextRange {
        let start = text.match_indices(needle).nth(nth).unwrap().0;
        let start = TextSize::from(u32::try_from(start).unwrap());
        TextRange::new(start, start + TextSize::of(needle))
    }

    #[test]
    fn file_lowering_diagnostics_flattens_all_owners() {
        let text = r#"
module m;
  int x = '{default: 0};
  initial begin : blk
    int y = '{default: 0};
  end
  task automatic t;
    int z = '{default: 0};
  endtask
  struct { logic a; } value;
endmodule
"#;
        let db = db_with_files(text, None);

        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));

        let pattern_ranges = diagnostics
            .iter()
            .filter(|diag| diag.syntax_kind == SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION)
            .map(|diag| diag.range.expect("range must be resolved"))
            .collect::<FxHashSet<_>>();
        assert_eq!(
            pattern_ranges,
            FxHashSet::from_iter([
                range_of_nth(text, "'{default: 0}", 0),
                range_of_nth(text, "'{default: 0}", 1),
                range_of_nth(text, "'{default: 0}", 2),
            ]),
            "module, block, and subroutine expressions must all be reported: {diagnostics:?}"
        );

        let struct_range = diagnostics
            .iter()
            .find(|diag| diag.syntax_kind == SyntaxKind::STRUCT_TYPE)
            .expect("struct data type should be diagnosed")
            .range
            .expect("range must be resolved");
        assert_eq!(struct_range, range_of(text, "struct { logic a; }"));
    }

    #[test]
    fn include_buffer_diagnostic_falls_back_to_owner_range() {
        let text = "module m;\n`include \"defs.vh\"\nendmodule\n";
        let db = db_with_files(text, Some("struct { logic a; } value;\n"));

        let diagnostics = db.file_lowering_diagnostics(HirFileId::File(TOP));
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.syntax_kind == SyntaxKind::STRUCT_TYPE)
            .unwrap_or_else(|| {
                panic!("include-buffer struct type should be diagnosed: {diagnostics:?}")
            });

        // The struct lives in an included buffer and has no root-buffer range;
        // the strategy falls back to a zero-width marker at the owner start.
        let range = diagnostic.range.expect("range must be resolved");
        assert_eq!(range, TextRange::empty(TextSize::new(0)));
    }

    #[test]
    fn range_resolution_prefers_explicit_range() {
        let tree = SyntaxTree::from_text(
            "module m;\n  int x = '{default: 0};\nendmodule\n",
            "top.sv",
            "/repo/rtl/top.sv",
        );
        let explicit = TextRange::new(TextSize::new(10), TextSize::new(20));
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: Some(explicit),
            message: "unsupported expression",
        };

        assert_eq!(super::resolve_range(&tree, None, &diagnostic), explicit);
    }

    #[test]
    fn range_resolution_uses_file_start_without_owner() {
        let text = "module m;\n  int x = '{default: 0};\nendmodule\n";
        let tree = SyntaxTree::from_text(text, "top.sv", "/repo/rtl/top.sv");
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: None,
            message: "unsupported expression",
        };

        let range = super::resolve_range(&tree, None, &diagnostic);
        assert_eq!(range, TextRange::empty(TextSize::new(0)));
    }

    #[test]
    fn range_resolution_honors_owner_scope() {
        let text = "module a;\n  int x = '{default: 0};\nendmodule\nmodule b;\nendmodule\n";
        let tree = SyntaxTree::from_text(text, "top.sv", "/repo/rtl/top.sv");
        let diagnostic = LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: SyntaxKind::ASSIGNMENT_PATTERN_EXPRESSION,
            source: None,
            range: None,
            message: "unsupported expression",
        };

        // Restricting the search to module b (which has no such node) must
        // fall through to the owner start instead of grabbing module a's node.
        let owner_range = range_of(text, "module b;");
        let range = super::resolve_range(&tree, Some(owner_range), &diagnostic);
        assert_eq!(range, TextRange::empty(owner_range.start()));
    }
}
