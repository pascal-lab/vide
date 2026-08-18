use hir_def::{Ident, container::InFile, def_id::DefId, item_tree::ModuleHeader, owner::OwnerId};
use hir_ty::db::TyDb;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::FxHashMap;
use syntax::{SyntaxNodeExt, has_text_range::HasTextRange, token::TokenKindExt};
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb, navigation_target::nav_location,
};

pub(crate) mod build;

/// Precomputed cross-file resolution inputs for one index build: the `$unit`
/// scope, package design map, and (when requested) per-root module indexes.
///
/// Instantiation indexes are filled on first use. Jumping to a module's own
/// name only needs [`hir`]; it must not walk every preprocessor model.
pub(crate) struct SemanticSnapshotInputs {
    pub hir: triomphe::Arc<hir_def::pathres::ResolutionContext>,
}

impl SemanticSnapshotInputs {
    pub(crate) fn from_hir(
        hir: triomphe::Arc<hir_def::pathres::ResolutionContext>,
    ) -> triomphe::Arc<Self> {
        triomphe::Arc::new(Self { hir })
    }

    pub(crate) fn from_db(db: &dyn WorkspaceSymbolIndexDb) -> triomphe::Arc<Self> {
        Self::from_db_with_hir(db, hir_def::pathres::ResolutionContext::from_db(db))
    }

    pub(crate) fn from_db_with_hir(
        _db: &dyn WorkspaceSymbolIndexDb,
        hir: triomphe::Arc<hir_def::pathres::ResolutionContext>,
    ) -> triomphe::Arc<Self> {
        Self::from_hir(hir)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SemanticDefinitionRange {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnSide {
    /// The reference is the port side of a shorthand connection (`.name`).
    Port,
    /// The reference is the local side of a shorthand connection (`.name`).
    Local,
}

/// Context of a reference token inside a named port connection, computed at
/// index build time so rename and other reference consumers never re-resolve.
///
/// `paired` is `Some` exactly when the connection is a same-name connection
/// (the `.name` and the data identifier have the same text): for the name
/// side it is the local definition, for the data side it is the port
/// definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReferenceContext {
    Plain,
    /// The token is the `.name` of a named port connection.
    ConnName {
        /// Range of the data identifier, when the data is a simple identifier.
        ident_range: Option<TextRange>,
        /// Range from the name token start to the closing paren end.
        collapse_range: Option<TextRange>,
        /// No-parens shorthand connection (`.name`).
        shorthand: bool,
        /// The side of a shorthand connection this reference belongs to.
        side: ConnSide,
        /// Same-name connections: the local definition of the data identifier.
        paired: Option<DefId>,
    },
    /// The token is a simple identifier in the data position of a named port
    /// connection.
    ConnData {
        /// Range of the connection's `.name` token.
        name_range: TextRange,
        /// Range from the name token start to the closing paren end.
        collapse_range: Option<TextRange>,
        /// Same-name connections: the port definition of the name token.
        paired: Option<DefId>,
    },
}

impl ReferenceContext {
    /// The paired same-name connection definition, when the connection is
    /// same-name: the local def for name tokens, the port def for data
    /// tokens, and the counterpart def for shorthand references.
    pub(crate) fn paired(&self) -> Option<&DefId> {
        match self {
            ReferenceContext::Plain => None,
            ReferenceContext::ConnName { paired, .. }
            | ReferenceContext::ConnData { paired, .. } => paired.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticModuleDefinition {
    pub module_id: OwnerId,
    pub file_id: FileId,
    pub name: Ident,
    pub name_range: TextRange,
    pub full_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCallItem {
    pub file_id: FileId,
    pub name: String,
    pub full_range: TextRange,
    pub name_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCallEdge {
    pub caller: ModuleCallItem,
    pub callee: ModuleCallItem,
    pub call_range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleEdgeIndex {
    incoming_module_edges: FxHashMap<OwnerId, Box<[ModuleCallEdge]>>,
    outgoing_module_edges: FxHashMap<OwnerId, Box<[ModuleCallEdge]>>,
}

/// Module definitions contributed by one file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileModuleIndex {
    modules: Vec<SemanticModuleDefinition>,
}

/// Module edges contributed by one file: the outgoing edges of the file's
/// modules, with caller and callee ids so the merge can build both maps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileModuleEdges {
    edges: Vec<(OwnerId, OwnerId, ModuleCallEdge)>,
}

impl SemanticModuleDefinition {
    fn new(db: &dyn TyDb, module_id: OwnerId) -> Option<Self> {
        let source_file = module_id.file(db);
        let header = db
            .item_tree(source_file)
            .module_headers()
            .find(|header| header.owner() == module_id)?;
        Self::from_header(db, source_file, header)
    }

    fn from_header(db: &dyn TyDb, source_file: HirFileId, header: ModuleHeader) -> Option<Self> {
        let origin = db.source_projection(source_file).origin(header.source())?;
        let full_range = origin.full_range()?;
        let (file_id, name_range, full_range) =
            nav_location(db, source_file, origin.focus_range(), full_range)?;

        Some(Self {
            module_id: header.owner(),
            file_id,
            name: header.name().clone(),
            name_range: name_range.unwrap_or(full_range),
            full_range,
        })
    }

    fn call_item(&self) -> ModuleCallItem {
        ModuleCallItem {
            file_id: self.file_id,
            name: self.name.to_string(),
            full_range: self.full_range,
            name_range: self.name_range,
        }
    }
}

impl ModuleEdgeIndex {
    pub(crate) fn from_file_edges<'a>(
        file_edges: impl IntoIterator<Item = &'a FileModuleEdges>,
    ) -> Self {
        let mut incoming_module_edges: FxHashMap<OwnerId, Vec<ModuleCallEdge>> =
            FxHashMap::default();
        let mut outgoing_module_edges: FxHashMap<OwnerId, Vec<ModuleCallEdge>> =
            FxHashMap::default();
        for file_edges in file_edges {
            for (caller, callee, edge) in &file_edges.edges {
                push_unique_edge(outgoing_module_edges.entry(*caller).or_default(), edge.clone());
                push_unique_edge(incoming_module_edges.entry(*callee).or_default(), edge.clone());
            }
        }
        Self {
            incoming_module_edges: finish_edge_map(incoming_module_edges),
            outgoing_module_edges: finish_edge_map(outgoing_module_edges),
        }
    }

    pub(crate) fn incoming_module_edges(&self, module_id: OwnerId) -> &[ModuleCallEdge] {
        self.incoming_module_edges.get(&module_id).map_or(&[], |edges| edges.as_ref())
    }

    pub(crate) fn outgoing_module_edges(&self, module_id: OwnerId) -> &[ModuleCallEdge] {
        self.outgoing_module_edges.get(&module_id).map_or(&[], |edges| edges.as_ref())
    }
}

pub(crate) fn incoming_module_edges(
    db: &crate::analysis::AnalysisContext<'_>,
    file_id: FileId,
    name_range: TextRange,
) -> Vec<ModuleCallEdge> {
    module_edges(db, file_id, name_range, |index, module_id| index.incoming_module_edges(module_id))
}

pub(crate) fn outgoing_module_edges(
    db: &crate::analysis::AnalysisContext<'_>,
    file_id: FileId,
    name_range: TextRange,
) -> Vec<ModuleCallEdge> {
    module_edges(db, file_id, name_range, |index, module_id| index.outgoing_module_edges(module_id))
}

fn module_edges(
    db: &crate::analysis::AnalysisContext<'_>,
    file_id: FileId,
    name_range: TextRange,
    edges_for_index: impl Fn(&ModuleEdgeIndex, OwnerId) -> &[ModuleCallEdge],
) -> Vec<ModuleCallEdge> {
    let Some(module_id) = module_id_at_range(db, file_id, name_range) else {
        return Vec::new();
    };

    let mut edges = Vec::new();
    for source_root_id in db.workspace_source_root_ids().iter().copied() {
        let index = db.module_edges(source_root_id);
        edges.extend(edges_for_index(&index, module_id).iter().cloned());
    }
    sort_and_dedup_edges(&mut edges);
    edges
}

fn module_id_at_range(
    db: &crate::analysis::AnalysisContext<'_>,
    file_id: FileId,
    name_range: TextRange,
) -> Option<OwnerId> {
    let hir_file = HirFileId::File(file_id);
    let item_tree = db.item_tree(hir_file);
    let projection = db.source_projection(hir_file);
    item_tree.module_headers().find_map(|header| {
        let origin = projection.origin(header.source())?;
        let full_range = origin.full_range()?;
        let (_, focus, full) = nav_location(db.db, hir_file, origin.focus_range(), full_range)?;
        (focus.unwrap_or(full) == name_range).then_some(header.owner())
    })
}

fn instantiation_name_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    instantiation_range: TextRange,
) -> Option<TextRange> {
    let tree = db.parse_src_for_compilation(file_id);
    let root = tree.root();
    let mut offset = instantiation_range.start();

    while offset < instantiation_range.end() {
        let token = root.token_after_or_at_offset(offset)?;
        let range = token.text_range()?;
        if range.start() >= instantiation_range.end() {
            return None;
        }
        if token.kind().name_like() {
            return Some(range);
        }
        offset = range.end();
    }

    None
}

fn push_unique_edge(edges: &mut Vec<ModuleCallEdge>, edge: ModuleCallEdge) {
    if !edges.iter().any(|existing| existing == &edge) {
        edges.push(edge);
    }
}

fn finish_edge_map(
    edges_by_module: FxHashMap<OwnerId, Vec<ModuleCallEdge>>,
) -> FxHashMap<OwnerId, Box<[ModuleCallEdge]>> {
    edges_by_module
        .into_iter()
        .map(|(key, mut edges)| {
            sort_and_dedup_edges(&mut edges);
            (key, edges.into_boxed_slice())
        })
        .collect()
}

fn sort_and_dedup_edges(edges: &mut Vec<ModuleCallEdge>) {
    edges.sort_by_key(|edge| {
        (
            edge.caller.file_id.index(),
            edge.caller.name_range.start(),
            edge.callee.file_id.index(),
            edge.callee.name_range.start(),
            edge.call_range.start(),
        )
    });
    edges.dedup();
}

#[cfg(test)]
mod tests {
    use hir_def::symbol::NameContext;
    use hir_semantics::semantics::SemanticsImpl;
    use preproc_expand::file::HirFileId;
    use syntax::{
        SyntaxElement, SyntaxNodeExt, WalkEvent,
        ast::{self, AstNode},
        has_text_range::HasTextRange,
        token::TokenKindExt,
    };
    use triomphe::Arc;
    use utils::line_index::{TextRange, TextSize};

    use super::*;
    use crate::{
        ScopeVisibility,
        definitions::DefinitionClass,
        references::{
            ReferencesConfig,
            search::{SearchScope, search_references},
        },
        semantic_index::build::{
            ContainerCache, ScopeChainCache, definition_ranges_for, token_in_special_context,
        },
        semantic_target::{
            SemanticTarget, TargetIntent, preproc::emit_token_index,
            resolve_semantic_target_with_emitted,
        },
        test_utils::{setup_marked, setup_marked_files},
    };

    fn def_named_at(
        db: &crate::analysis::AnalysisContext<'_>,
        file_id: FileId,
        range: TextRange,
    ) -> DefId {
        let tree = db.parse(HirFileId::from(file_id));
        let token = tree
            .root()
            .token_at_offset(range.start())
            .find(|token| token.text_range() == Some(range))
            .expect("definition token");
        match DefinitionClass::resolve(db, file_id.into(), token).unique().expect("unique def") {
            DefinitionClass::Definition(def) => def,
            DefinitionClass::PortConnShorthand { port, .. } => port,
        }
    }

    fn workspace_refs(
        db: &crate::analysis::AnalysisContext<'_>,
        def: DefId,
    ) -> Vec<(FileId, TextRange, ReferenceContext)> {
        let scope =
            SearchScope::new(db.db, &def, ReferencesConfig::new(ScopeVisibility::Public, None));
        search_references(db, &def, scope)
            .into_iter()
            .flat_map(|(file_id, tokens)| {
                tokens
                    .into_iter()
                    .map(move |token| (file_id, token.range(), token.context().clone()))
            })
            .collect()
    }

    /// A non-structural (body-only) edit must drop a removed usage from the
    /// next search without mutating a previously observed name table.
    #[test]
    fn incremental_rebuild_drops_removed_reference() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            (
                "/child.sv",
                "module child;\n  logic /*marker:def*/a;\n  logic b;\n  always_comb b = /*marker:use*/a;\nendmodule\n",
            ),
            ("/top.sv", "module top;\n  child u();\nendmodule\n"),
        ]);
        let child_id = marked[0].0;
        let markers = &marked[0].2;
        let def_range = TextRange::new(markers["def"], markers["def"] + TextSize::of("a"));
        let db = host.ctx();
        let def = def_named_at(&db, child_id, def_range);
        let before_index = db.file_name_index(child_id);
        assert_eq!(workspace_refs(&db, def).len(), 1, "wire a has one usage");
        assert_eq!(before_index.occurrences("a").len(), 2);

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(
            child_id,
            "module child;\n  logic a;\n  logic b;\n  always_comb b = 1'b0;\nendmodule\n",
        ));
        host.apply_change(change);
        let db = host.ctx();

        assert!(
            workspace_refs(&db, def).is_empty(),
            "removing the only usage must drop the reference"
        );
        assert_eq!(
            before_index.occurrences("a").len(),
            2,
            "a name-table snapshot held by a caller must not be mutated in place"
        );
    }

    #[test]
    fn request_resolution_context_reuses_body_edits_and_rebuilds_structural_edits() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, file_id, clean, _) = setup_marked("module top; logic a; endmodule\n");
        let before = host.ctx().semantic_snapshot_inputs();

        let mut body_edit = Change::new();
        body_edit.add_changed_file(ChangedFile::create(
            file_id,
            format!("{clean} // body-only\n").as_str(),
        ));
        host.apply_change(body_edit);
        let after_body = host.ctx().semantic_snapshot_inputs();
        assert!(
            Arc::ptr_eq(&before, &after_body),
            "position-free structure is unchanged, so the context must be reused"
        );

        let mut structural_edit = Change::new();
        structural_edit
            .add_changed_file(ChangedFile::create(file_id, "module renamed; logic a; endmodule\n"));
        host.apply_change(structural_edit);
        let after_structure = host.ctx().semantic_snapshot_inputs();
        assert!(
            !Arc::ptr_eq(&after_body, &after_structure),
            "a changed declaration must invalidate the project resolution context"
        );
    }

    #[test]
    fn body_edit_of_a_file_with_includes_reuses_resolution() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            ("/defs.svh", "`define WIDTH 8\n"),
            ("/top.sv", "`include \"defs.svh\"\nmodule top; logic a; endmodule\n"),
        ]);
        let top = marked[1].0;
        let before = host.ctx().semantic_snapshot_inputs();

        let mut body_edit = Change::new();
        body_edit.add_changed_file(ChangedFile::create(
            top,
            "`include \"defs.svh\"\nmodule top; logic a; endmodule\n// body-only\n",
        ));
        host.apply_change(body_edit);
        let after_body = host.ctx().semantic_snapshot_inputs();
        assert!(
            Arc::ptr_eq(&before, &after_body),
            "an include file's body-only comment must not rebuild resolution via item_tree"
        );
    }

    #[test]
    fn recorded_include_dependency_invalidates_the_parsed_root() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            ("/defs.svh", "`define UNIT_NAME top\n"),
            ("/top.sv", "`include \"defs.svh\"\nmodule `UNIT_NAME; endmodule\n"),
        ]);
        let defs = marked[0].0;
        let top = marked[1].0;
        let db = host.ctx();
        db.store.record_parse_dependencies(top, Arc::from(vec![top, defs]));
        let before = db.semantic_snapshot_inputs();

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(defs, "`define UNIT_NAME renamed\n"));
        host.apply_change(change);
        let after = host.ctx().semantic_snapshot_inputs();

        assert!(
            !Arc::ptr_eq(&before, &after),
            "an emitted include dependency must invalidate the parsed root's structure products"
        );
    }

    #[test]
    fn request_file_index_reuses_unrelated_edits_and_rebuilds_its_file() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            ("/a.sv", "module a; logic x; endmodule\n"),
            ("/b.sv", "module b; logic y; endmodule\n"),
        ]);
        let a = marked[0].0;
        let b = marked[1].0;
        let before = host.ctx().file_name_index(b);

        let mut unrelated = Change::new();
        unrelated.add_changed_file(ChangedFile::create(
            a,
            "module a; logic x; endmodule // body-only\n",
        ));
        host.apply_change(unrelated);
        let after_unrelated = host.ctx().file_name_index(b);
        assert!(Arc::ptr_eq(&before, &after_unrelated));

        let mut own_edit = Change::new();
        own_edit.add_changed_file(ChangedFile::create(
            b,
            "module b; logic y; endmodule // own body-only\n",
        ));
        host.apply_change(own_edit);
        let after_own_edit = host.ctx().file_name_index(b);
        assert!(!Arc::ptr_eq(&after_unrelated, &after_own_edit));
    }

    /// Two body edits without a request between them must both be visible.
    /// A replacing dirty set would drop the first file's dirtiness and leave
    /// its removed reference in the merged index.
    #[test]
    fn consecutive_body_edits_both_reach_the_merged_index() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            (
                "/a.sv",
                "module a;\n  logic /*marker:x*/x;\n  logic y;\n  always_comb y = x;\nendmodule\n",
            ),
            (
                "/b.sv",
                "module b;\n  logic /*marker:p*/p;\n  logic q;\n  always_comb q = p;\nendmodule\n",
            ),
        ]);
        let a = marked[0].0;
        let b = marked[1].0;
        let x_range = TextRange::new(marked[0].2["x"], marked[0].2["x"] + TextSize::of("x"));
        let p_range = TextRange::new(marked[1].2["p"], marked[1].2["p"] + TextSize::of("p"));
        let db = host.ctx();
        let def_x = def_named_at(&db, a, x_range);
        let def_p = def_named_at(&db, b, p_range);
        assert_eq!(workspace_refs(&db, def_x).len(), 1);
        assert_eq!(workspace_refs(&db, def_p).len(), 1);

        let mut first = Change::new();
        first.add_changed_file(ChangedFile::create(
            a,
            "module a;\n  logic x;\n  logic y;\n  always_comb y = 1'b0;\nendmodule\n",
        ));
        host.apply_change(first);

        let mut second = Change::new();
        second.add_changed_file(ChangedFile::create(
            b,
            "module b;\n  logic p;\n  logic q;\n  always_comb q = 1'b0;\nendmodule\n",
        ));
        host.apply_change(second);

        let db = host.ctx();
        assert!(
            workspace_refs(&db, def_x).is_empty(),
            "the first edit must not be dropped when a second edit arrives before a request"
        );
        assert!(workspace_refs(&db, def_p).is_empty(), "the second edit must still be applied");
    }

    /// The container stack must agree with `find_container` for every
    /// name-like token of a file exercising modules, blocks, subroutines,
    /// explicit generate blocks, single-member generate branches and
    /// instantiations. This is the safety net for the dispatch that mirrors
    /// `source_to_def::container_to_def`.
    #[test]
    fn container_stack_matches_find_container_for_every_token() {
        let text = r#"
`define TWO_MODULES module first; endmodule module second; endmodule
`TWO_MODULES
module top(input logic clk);
  logic sig;
  always_ff @(posedge clk) begin
    if (sig) begin
      logic inner;
    end
  end
  generate
    if (1) begin : gen_if
      wire g;
    end
  endgenerate
  function automatic logic f();
    return sig;
  endfunction
  sub u_sub();
endmodule
"#;
        let (host, file_id, _clean, _markers) = setup_marked(text);
        let db = host.ctx();
        let hir_file_id = HirFileId::from(file_id);
        let tree = db.parse(hir_file_id);
        let root = tree.root();
        let macro_modules = root
            .elem_preorder()
            .filter_map(|event| match event {
                WalkEvent::Enter(SyntaxElement::Node(node)) => {
                    ast::ModuleDeclaration::cast(node).map(|module| module.syntax())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            macro_modules.windows(2).any(|modules| {
                modules[0].kind() == modules[1].kind()
                    && modules[0].text_range() == modules[1].text_range()
                    && modules[0] != modules[1]
            }),
            "macro expansion should contain distinct module nodes with the same display identity"
        );
        let sema = SemanticsImpl::new(db.db);
        let mut containers = ContainerCache::new();
        for event in root.elem_preorder() {
            match event {
                WalkEvent::Enter(SyntaxElement::Node(_)) => {}
                WalkEvent::Leave(SyntaxElement::Node(_)) => {}
                WalkEvent::Enter(SyntaxElement::Token(token)) => {
                    if !token.kind().name_like() {
                        continue;
                    }
                    let cached = containers.container_for(&sema, hir_file_id, token.parent);
                    let expected =
                        sema.container_for_node(hir_file_id, token.parent).unwrap_or_else(|| {
                            db.owner_table(hir_file_id).file_owner().expect("file owner")
                        });
                    if cached != expected {
                        eprintln!("cached owner={cached:?}");
                        eprintln!("expected owner={expected:?}");
                    }
                    assert_eq!(cached, expected, "container mismatch at {:?}", token.raw_text());
                }
                WalkEvent::Leave(SyntaxElement::Token(_)) => {}
            }
        }
    }

    /// The fast path must agree with the full heuristic chain for every
    /// token: `token_in_special_context` has to cover exactly the syntax
    /// positions where `DefinitionClass::resolve_in` diverges from plain
    /// value-name resolution. The fixture exercises member accesses, scoped
    /// names, packages, checkers, module-like declarations, hierarchy /
    /// primitive instantiations, named port connections, named types, package
    /// imports and a macro emitting a member access.
    #[test]
    fn fast_path_agrees_with_full_resolution_chain_for_every_token() {
        let text = r#"
`define M(a) a.x
package pkg;
  logic field;
endpackage

checker chk(input logic a);
endchecker

module sub(input logic in, output logic out);
  logic internal;
  assign out = in & internal;
endmodule

module top(input logic clk, input logic [3:0] data);
  logic sig;
  wire [3:0] w;
  pkg::field f_field;
  initial begin
    sig = clk;
    `M(sig)
  end
  sub u_sub(.in(sig), .out(w));
  and g1(w, sig, clk);
  chk c1(.a(sig));
  import pkg::*;
endmodule
"#;
        let (host, file_id, _clean, _markers) = setup_marked(text);
        let db = host.ctx();
        let context = SemanticSnapshotInputs::from_db(db.db);
        let hir_file_id = HirFileId::from(file_id);
        let tree = db.parse(hir_file_id);
        let root = tree.root();
        let sema = SemanticsImpl::new(db.db);
        let mut containers = ContainerCache::new();
        let mut chains = ScopeChainCache::new();
        let mut checked = 0usize;
        for event in root.elem_preorder() {
            if let WalkEvent::Enter(SyntaxElement::Token(token)) = event {
                if !token.kind().name_like() {
                    continue;
                }
                checked += 1;
                let container = containers.container_for(&sema, hir_file_id, token.parent);
                let chosen = if token_in_special_context(token) {
                    DefinitionClass::resolve_in(
                        db.db,
                        &context,
                        hir_file_id,
                        token,
                        Some(container),
                    )
                    .unique()
                } else {
                    let chain = chains.chain_for(db.db, container);
                    sema.nameres_ident_in_scopes_at(hir_file_id, token, NameContext::Value, &chain)
                        .map(DefinitionClass::Definition)
                        .unique()
                };
                let full = DefinitionClass::resolve_in(
                    db.db,
                    &context,
                    hir_file_id,
                    token,
                    Some(container),
                )
                .unique();
                assert_eq!(
                    chosen,
                    full,
                    "fast path diverges at {:?} (parent={:?}, special={})",
                    token.raw_text(),
                    token.parent.kind(),
                    token_in_special_context(token)
                );
            }
        }
        assert!(checked > 20, "test should exercise a non-trivial token set");
    }

    /// Named port connections must record their shape (name/data roles,
    /// collapse ranges, shorthand sides and same-name pairing) on the
    /// references, so rename never re-resolves or re-parses.
    #[test]
    fn reference_contexts_capture_named_connection_shapes() {
        let text = r#"
module child(input /*marker:child_a*/a, input /*marker:child_b*/b);
endmodule
module top;
  logic /*marker:local_a*/a;
  logic /*marker:local_b*/b;
  logic /*marker:local_c*/c;
  logic /*marker:plain_c*/d;
  assign d = /*marker:plain*/c;
  child u(/*marker:same_name*/.a(/*marker:same_name_data*/a), /*marker:other_name*/.b(/*marker:other_data*/c));
  child v(/*marker:shorthand*/.b);
endmodule
"#;
        let (host, file_id, _clean, markers) = setup_marked(text);
        let db = host.ctx();

        let range_at = |marker: &str| {
            let start = markers[marker];
            let end = markers[marker] + TextSize::of("a");
            TextRange::new(start, end)
        };
        // Conn name markers sit on the leading dot; the name token follows it.
        let conn_name_at = |marker: &str| {
            let start = markers[marker] + TextSize::of(".");
            TextRange::new(start, start + TextSize::of("a"))
        };
        let def_range = |marker: &str| range_at(marker);
        let refs_of = |def_marker: &str| {
            let def = def_named_at(&db, file_id, def_range(def_marker));
            workspace_refs(&db, def)
        };
        let reference = |def_marker: &str, range: TextRange| -> (TextRange, ReferenceContext) {
            let refs = refs_of(def_marker);
            let found = refs
                .iter()
                .find(|(_, found, _)| *found == range)
                .unwrap_or_else(|| panic!("missing reference at {range:?} for {def_marker}"));
            (range, found.2.clone())
        };
        let paired_is = |paired: DefId, marker: &str| {
            definition_ranges_for(db.db, paired)
                .iter()
                .any(|range| range.file_id == file_id && range.range == def_range(marker))
        };

        // Same-name connection `.a(a)`: the name token pairs the local def,
        // the data token pairs the port def, both share the collapse range.
        let same_name_range = conn_name_at("same_name");
        let same_name_data_range = range_at("same_name_data");
        let collapse =
            TextRange::new(same_name_range.start(), same_name_data_range.end() + TextSize::of(")"));
        let name_ref = reference("child_a", conn_name_at("same_name"));
        let ReferenceContext::ConnName { ident_range, collapse_range, shorthand, side, paired } =
            &name_ref.1
        else {
            panic!("same-name name token should be ConnName: {:?}", name_ref.1);
        };
        assert_eq!(ident_range, &Some(same_name_data_range));
        assert_eq!(collapse_range, &Some(collapse));
        assert!(!shorthand);
        assert_eq!(side, &ConnSide::Port);
        let paired = *paired.as_ref().expect("same-name conn should pair the local def");
        assert!(paired_is(paired, "local_a"), "paired local def should be top.a");
        let data_ref = reference("local_a", range_at("same_name_data"));
        let ReferenceContext::ConnData { name_range, collapse_range, paired } = &data_ref.1 else {
            panic!("same-name data token should be ConnData: {:?}", data_ref.1);
        };
        assert_eq!(name_range, &same_name_range);
        assert_eq!(collapse_range, &Some(collapse));
        let paired = *paired.as_ref().expect("same-name conn should pair the port def");
        assert!(paired_is(paired, "child_a"), "paired port def should be child.a");

        // Non-same-name connection `.b(c)`: shape is recorded, no pairing.
        let name_ref = reference("child_b", conn_name_at("other_name"));
        let ReferenceContext::ConnName { ident_range, paired, .. } = &name_ref.1 else {
            panic!("non-same-name name token should be ConnName: {:?}", name_ref.1);
        };
        assert_eq!(ident_range, &Some(range_at("other_data")));
        assert_eq!(paired, &None);
        let data_ref = reference("local_c", range_at("other_data"));
        let ReferenceContext::ConnData { name_range, paired, .. } = &data_ref.1 else {
            panic!("non-same-name data token should be ConnData: {:?}", data_ref.1);
        };
        assert_eq!(name_range, &conn_name_at("other_name"));
        assert_eq!(paired, &None);

        // Shorthand `.b`: one reference in each side's group.
        let port_ref = reference("child_b", conn_name_at("shorthand"));
        let ReferenceContext::ConnName { collapse_range, shorthand, side, paired, .. } =
            &port_ref.1
        else {
            panic!("shorthand port reference should be ConnName: {:?}", port_ref.1);
        };
        assert!(shorthand);
        assert_eq!(collapse_range, &None);
        assert_eq!(side, &ConnSide::Port);
        let paired = *paired.as_ref().expect("shorthand should pair the local def");
        assert!(paired_is(paired, "local_b"), "shorthand port side should pair top.b");
        let local_ref = reference("local_b", conn_name_at("shorthand"));
        let ReferenceContext::ConnName { side, paired, .. } = &local_ref.1 else {
            panic!("shorthand local reference should be ConnName: {:?}", local_ref.1);
        };
        assert_eq!(side, &ConnSide::Local);
        let paired = *paired.as_ref().expect("shorthand should pair the port def");
        assert!(paired_is(paired, "child_b"), "shorthand local side should pair child.b");

        // Plain references stay Plain.
        let plain = reference("local_c", range_at("plain"));
        assert_eq!(plain.1, ReferenceContext::Plain);
    }

    #[test]
    fn semantic_index_skips_preprocessor_owned_identifiers() {
        let text = r#"
`define BODY(/*marker:param*/x) /*marker:body*/x
module top;
  wire /*marker:def*/x;
  assign y = /*marker:ordinary*/x;
  assign y = `BODY(/*marker:arg*/x);
endmodule
"#;
        let (host, file_id, _clean, markers) = setup_marked(text);
        let db = host.ctx();
        let tree = db.parse(HirFileId::from(file_id));
        let root = tree.root();
        let emitted = emit_token_index(root);
        for marker in ["param", "body"] {
            let target = resolve_semantic_target_with_emitted(
                db.db,
                file_id,
                markers[marker],
                Some(root),
                crate::token::navigation_precedence,
                Some(&emitted),
            )
            .unique_for_intent(TargetIntent::FindReferences);
            assert!(
                matches!(target, Some(SemanticTarget::PreprocMacro(_))),
                "{marker} must remain owned by the preprocessor: {target:?}"
            );
        }
        let definition_range = TextRange::new(markers["def"], markers["def"] + TextSize::of("x"));
        let preproc_ranges = [
            TextRange::new(markers["param"], markers["param"] + TextSize::of("x")),
            TextRange::new(markers["body"], markers["body"] + TextSize::of("x")),
        ];
        let def = def_named_at(&db, file_id, definition_range);
        let refs = workspace_refs(&db, def);

        assert!(
            refs.iter().all(|(_, range, _)| !preproc_ranges.contains(range)),
            "preprocessor-owned x tokens must not become HDL references: {refs:?}"
        );
        assert!(refs.iter().any(|(_, range, _)| {
            *range == TextRange::new(markers["ordinary"], markers["ordinary"] + TextSize::of("x"))
        }));
        assert!(refs.iter().any(|(_, range, _)| {
            *range == TextRange::new(markers["arg"], markers["arg"] + TextSize::of("x"))
        }));
    }
}
