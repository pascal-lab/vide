use base_db::{source_db::SourceRootDb, source_root::SourceRootId};
use hir_def::{Ident, container::InFile, def_id::DefId, item_tree::ModuleHeader, owner::OwnerId};
use hir_ty::db::TyDb;
use preproc_expand::{db::PreprocDb, file::HirFileId, macro_file::macro_files_for_file};
use rustc_hash::{FxHashMap, FxHashSet};
use syntax::{
    SyntaxNodeExt, TokenKind, has_text_range::HasTextRange, ptr::SyntaxTokenPtr,
    token::TokenKindExt,
};
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    db::{
        root_db::RootDb,
        workspace_symbol_index_db::{
            WorkspaceSymbolIndexDb, source_root_module_index_for_root,
            source_root_module_edge_index_for_root,
        },
    },
    navigation_target::nav_location,
    references::ReferenceCategory,
};

mod build;
use build::definition_ranges_for;

/// Precomputed cross-file resolution inputs for one index build: the `$unit`
/// scope, package design map, top-level module index, and per-root module
/// indexes. Computed once per request so the per-file nameres never reads the
/// O(project) global queries through salsa.
pub(crate) struct IndexResolutionContext {
    pub hir: triomphe::Arc<hir_def::pathres::ResolutionContext>,
    pub module_indexes: triomphe::Arc<[(SourceRootId, Arc<ModuleIndex>)]>,
}

impl IndexResolutionContext {
    pub(crate) fn from_db(db: &dyn WorkspaceSymbolIndexDb) -> triomphe::Arc<Self> {
        Self::from_db_with_hir(db, hir_def::pathres::ResolutionContext::from_db(db))
    }

    pub(crate) fn from_db_with_hir(
        db: &dyn WorkspaceSymbolIndexDb,
        hir: triomphe::Arc<hir_def::pathres::ResolutionContext>,
    ) -> triomphe::Arc<Self> {
        let module_indexes: Vec<_> = db
            .workspace_source_root_ids()
            .into_iter()
            .map(|root| (root, source_root_module_index_for_root(db, root)))
            .collect();
        triomphe::Arc::new(Self {
            hir,
            module_indexes: triomphe::Arc::from(module_indexes),
        })
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
pub(crate) struct SemanticReference {
    pub file_id: FileId,
    pub range: TextRange,
    pub category: ReferenceCategory,
    pub ptr: SyntaxTokenPtr,
    pub context: ReferenceContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticReferenceGroup {
    pub name: String,
    pub definition_ranges: Box<[SemanticDefinitionRange]>,
    pub references: Box<[SemanticReference]>,
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
pub struct ModuleIndex {
    modules_by_name: FxHashMap<Ident, Box<[SemanticModuleDefinition]>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceIndex {
    references_by_definition: FxHashMap<DefId, SemanticReferenceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModuleEdgeIndex {
    incoming_module_edges: FxHashMap<OwnerId, Box<[ModuleCallEdge]>>,
    outgoing_module_edges: FxHashMap<OwnerId, Box<[ModuleCallEdge]>>,
}

/// Per-file slice of the semantic index: reference groups without the
/// cross-file definition ranges, which are computed once at merge time.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileSemanticIndex {
    groups: FxHashMap<DefId, FileReferenceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReferenceGroup {
    name: String,
    references: Vec<SemanticReference>,
}

impl FileReferenceGroup {
    pub(crate) fn references(&self) -> &[SemanticReference] {
        &self.references
    }
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

#[derive(Debug)]
struct SemanticReferenceGroupBuilder {
    name: String,
    definition_ranges: Vec<SemanticDefinitionRange>,
    references: Vec<SemanticReference>,
}

impl ModuleIndex {
    /// Merges the per-file module indexes of a source root.
    pub(crate) fn for_source_root(
        db: &dyn WorkspaceSymbolIndexDb,
        source_root_id: SourceRootId,
    ) -> Self {
        let source_root = db.source_root(source_root_id);
        let mut modules_by_name: FxHashMap<Ident, Vec<SemanticModuleDefinition>> =
            FxHashMap::default();

        let mut hir_files = Vec::new();
        for file_id in source_root.iter() {
            hir_files.push(HirFileId::File(file_id));
            hir_files.extend(macro_files_for_file(db, file_id).into_iter().map(HirFileId::Macro));
        }
        hir_files.sort_unstable();
        hir_files.dedup();

        for hir_file_id in hir_files {
            let item_tree = db.item_tree(hir_file_id);
            for header in
                item_tree.module_headers().filter(|header| header.kind().is_instantiable())
            {
                let Some(module) = SemanticModuleDefinition::from_header(db, hir_file_id, header)
                else {
                    continue;
                };
                modules_by_name.entry(module.name.clone()).or_default().push(module);
            }
        }

        Self {
            modules_by_name: modules_by_name
                .into_iter()
                .map(|(name, mut modules)| {
                    modules
                        .sort_by_key(|module| (module.file_id.index(), module.name_range.start()));
                    modules.dedup_by(|lhs, rhs| {
                        lhs.module_id == rhs.module_id
                            || (lhs.file_id == rhs.file_id && lhs.name_range == rhs.name_range)
                    });
                    (name, modules.into_boxed_slice())
                })
                .collect(),
        }
    }

    pub(crate) fn module_definitions(&self, name: &Ident) -> &[SemanticModuleDefinition] {
        self.modules_by_name.get(name).map_or(&[], |modules| modules.as_ref())
    }

    fn module_definition_at(
        &self,
        file_id: FileId,
        name_range: TextRange,
    ) -> Option<&SemanticModuleDefinition> {
        self.all_module_definitions()
            .find(|module| module.file_id == file_id && module.name_range == name_range)
    }

    fn all_module_definitions(&self) -> impl Iterator<Item = &SemanticModuleDefinition> {
        self.modules_by_name.values().flat_map(|modules| modules.iter())
    }
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

impl ReferenceIndex {
    /// Merges pre-resolved per-file indexes. The per-file indexes are read by
    /// the caller so an incremental rebuild can reuse the cached indexes of
    /// unchanged files instead of revalidating every file.
    pub(crate) fn from_file_indexes(
        db: &dyn WorkspaceSymbolIndexDb,
        file_indexes: &FxHashMap<FileId, Arc<FileSemanticIndex>>,
    ) -> Self {
        let mut references_by_definition: FxHashMap<DefId, SemanticReferenceGroupBuilder> =
            FxHashMap::default();
        for file_index in file_indexes.values() {
            for (definition, group) in &file_index.groups {
                let builder = references_by_definition.entry(*definition).or_insert_with(|| {
                    SemanticReferenceGroupBuilder {
                        name: group.name.clone(),
                        definition_ranges: definition_ranges_for(db, *definition),
                        references: Vec::new(),
                    }
                });
                builder.references.extend(group.references.iter().cloned());
            }
        }

        ReferenceIndex {
            references_by_definition: references_by_definition
                .into_iter()
                .map(|(key, group)| (key, group.finish()))
                .collect(),
        }
    }

    pub(crate) fn references_for_definition(
        &self,
        definition: DefId,
    ) -> Option<&SemanticReferenceGroup> {
        self.references_by_definition.get(&definition)
    }

    /// Replaces one file's contribution in place. Definitions already in the
    /// index keep their cached name and definition ranges, so an incremental
    /// rebuild never re-projects origins for the whole project.
    pub(crate) fn patch_file(
        &mut self,
        db: &dyn WorkspaceSymbolIndexDb,
        file_id: FileId,
        old_file_index: &FileSemanticIndex,
        new_file_index: &FileSemanticIndex,
    ) {
        let map = &mut self.references_by_definition;
        let mut affected: FxHashSet<DefId> = old_file_index.groups.keys().copied().collect();
        affected.extend(new_file_index.groups.keys().copied());

        for definition in affected {
            match new_file_index.groups.get(&definition) {
                Some(new_group) => {
                    let group = map.entry(definition).or_insert_with(|| SemanticReferenceGroup {
                        name: new_group.name.clone(),
                        definition_ranges: definition_ranges_for(db, definition).into_boxed_slice(),
                        references: Box::default(),
                    });
                    let mut references: Vec<_> = group
                        .references
                        .iter()
                        .filter(|reference| reference.file_id != file_id)
                        .cloned()
                        .collect();
                    references.extend(new_group.references.iter().cloned());
                    group.references = references.into_boxed_slice();
                }
                None => {
                    if let Some(group) = map.get_mut(&definition) {
                        let references: Vec<_> = group
                            .references
                            .iter()
                            .filter(|reference| reference.file_id != file_id)
                            .cloned()
                            .collect();
                        if references.is_empty() {
                            map.remove(&definition);
                        } else {
                            group.references = references.into_boxed_slice();
                        }
                    }
                }
            }
        }

    }

    #[cfg(test)]
    pub(crate) fn reference_groups_named(&self, name: &str) -> Vec<&SemanticReferenceGroup> {
        self.references_by_definition.values().filter(|group| group.name == name).collect()
    }
}

impl ModuleEdgeIndex {
    /// Merges the per-file module edges of a source root.
    pub(crate) fn for_source_root(
        db: &dyn WorkspaceSymbolIndexDb,
        source_root_id: SourceRootId,
    ) -> Self {
        let source_root = db.source_root(source_root_id);
        let mut incoming_module_edges: FxHashMap<OwnerId, Vec<ModuleCallEdge>> =
            FxHashMap::default();
        let mut outgoing_module_edges: FxHashMap<OwnerId, Vec<ModuleCallEdge>> =
            FxHashMap::default();

        for file_id in source_root.iter() {
            db.unwind_if_revision_cancelled();
            for (caller, callee, edge) in &db.file_module_edges(file_id).edges {
                push_unique_edge(outgoing_module_edges.entry(*caller).or_default(), edge.clone());
                push_unique_edge(incoming_module_edges.entry(*callee).or_default(), edge.clone());
            }
        }

        ModuleEdgeIndex {
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

impl SemanticReferenceGroupBuilder {
    fn finish(self) -> SemanticReferenceGroup {
        SemanticReferenceGroup {
            name: self.name,
            definition_ranges: self.definition_ranges.into_boxed_slice(),
            references: self.references.into_boxed_slice(),
        }
    }
}

pub(crate) fn incoming_module_edges(
    db: &RootDb,
    file_id: FileId,
    name_range: TextRange,
) -> Vec<ModuleCallEdge> {
    module_edges(db, file_id, name_range, |index, module_id| index.incoming_module_edges(module_id))
}

pub(crate) fn outgoing_module_edges(
    db: &RootDb,
    file_id: FileId,
    name_range: TextRange,
) -> Vec<ModuleCallEdge> {
    module_edges(db, file_id, name_range, |index, module_id| index.outgoing_module_edges(module_id))
}

fn module_edges(
    db: &RootDb,
    file_id: FileId,
    name_range: TextRange,
    edges_for_index: impl Fn(&ModuleEdgeIndex, OwnerId) -> &[ModuleCallEdge],
) -> Vec<ModuleCallEdge> {
    let Some(module_id) = module_id_at_range(db, file_id, name_range) else {
        return Vec::new();
    };

    let mut edges = Vec::new();
    for source_root_id in db.workspace_source_root_ids().iter().copied() {
        let index = source_root_module_edge_index_for_root(db, source_root_id);
        edges.extend(edges_for_index(&index, module_id).iter().cloned());
    }
    sort_and_dedup_edges(&mut edges);
    edges
}

fn module_id_at_range(db: &RootDb, file_id: FileId, name_range: TextRange) -> Option<OwnerId> {
    let module_index = source_root_module_index_for_root(db, db.source_root_id(file_id));
    module_index.module_definition_at(file_id, name_range).map(|module| module.module_id)
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

fn token_precedence(kind: TokenKind) -> usize {
    crate::token::name_precedence(kind)
}

#[cfg(test)]
mod tests {
    use hir_def::symbol::NameContext;
    use hir_semantics::semantics::SemanticsImpl;
    use preproc_expand::file::HirFileId;
    use syntax::{
        SyntaxElement, WalkEvent,
        ast::{self, AstNode},
        has_text_range::HasTextRange,
        token::TokenKindExt,
    };
    use utils::line_index::{TextRange, TextSize};

    use super::*;
    use crate::{
        db::workspace_symbol_index_db::source_root_reference_index_for_root,
        definitions::DefinitionClass,
        semantic_index::build::{ContainerCache, ScopeChainCache, token_in_special_context},
        semantic_target::{
            SemanticTarget, TargetIntent, preproc::emit_token_index,
            resolve_semantic_target_with_emitted,
        },
        test_utils::{setup_marked, setup_marked_files},
    };

    /// A non-structural (body-only) edit must be handled by the incremental
    /// rebuild path: the changed file is re-indexed and a removed reference is
    /// dropped from the merged index, without touching the other file.
    #[test]
    fn incremental_rebuild_drops_removed_reference() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, marked) = setup_marked_files(&[
            (
                "/child.sv",
                "module child;\n  logic a;\n  logic b;\n  always_comb b = a;\nendmodule\n",
            ),
            ("/top.sv", "module top;\n  child u();\nendmodule\n"),
        ]);
        let db = host.raw_db();

        let before = source_root_reference_index_for_root(db, SourceRootId(0));
        assert_eq!(before.reference_groups_named("a").len(), 1, "wire a has one usage");

        let child_id = marked[0].0;
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::create(
            child_id,
            "module child;\n  logic a;\n  logic b;\n  always_comb b = 1'b0;\nendmodule\n",
        ));
        host.apply_change(change);
        let db = host.raw_db();

        let after = source_root_reference_index_for_root(db, SourceRootId(0));
        assert!(
            after.reference_groups_named("a").is_empty(),
            "removing the only usage must drop wire a's group"
        );
        assert_eq!(
            before.reference_groups_named("a").len(),
            1,
            "an index snapshot held by a caller must not be mutated in place"
        );
    }

    #[test]
    fn request_resolution_context_reuses_body_edits_and_rebuilds_structural_edits() {
        use base_db::change::Change;
        use vfs::ChangedFile;

        let (mut host, file_id, clean, _) = setup_marked("module top; logic a; endmodule\n");
        let before = host.raw_db().index_resolution_context();

        let mut body_edit = Change::new();
        body_edit.add_changed_file(ChangedFile::create(
            file_id,
            format!("{clean} // body-only\n").as_str(),
        ));
        host.apply_change(body_edit);
        let after_body = host.raw_db().index_resolution_context();
        assert!(
            Arc::ptr_eq(&before, &after_body),
            "position-free structure is unchanged, so the context must be reused"
        );

        let mut structural_edit = Change::new();
        structural_edit.add_changed_file(ChangedFile::create(
            file_id,
            "module renamed; logic a; endmodule\n",
        ));
        host.apply_change(structural_edit);
        let after_structure = host.raw_db().index_resolution_context();
        assert!(
            !Arc::ptr_eq(&after_body, &after_structure),
            "a changed declaration must invalidate the project resolution context"
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
        let before = host.raw_db().request_file_semantic_index(b);

        let mut unrelated = Change::new();
        unrelated.add_changed_file(ChangedFile::create(
            a,
            "module a; logic x; endmodule // body-only\n",
        ));
        host.apply_change(unrelated);
        let after_unrelated = host.raw_db().request_file_semantic_index(b);
        assert!(Arc::ptr_eq(&before, &after_unrelated));

        let mut own_edit = Change::new();
        own_edit.add_changed_file(ChangedFile::create(
            b,
            "module b; logic y; endmodule // own body-only\n",
        ));
        host.apply_change(own_edit);
        let after_own_edit = host.raw_db().request_file_semantic_index(b);
        assert!(!Arc::ptr_eq(&after_unrelated, &after_own_edit));
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
        let db = host.raw_db();
        let context = IndexResolutionContext::from_db(db);
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
        let sema = SemanticsImpl::new(db);
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
        let db = host.raw_db();
        let context = IndexResolutionContext::from_db(db);
        let hir_file_id = HirFileId::from(file_id);
        let tree = db.parse(hir_file_id);
        let root = tree.root();
        let sema = SemanticsImpl::new(db);
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
                    DefinitionClass::resolve_in(db, &context, hir_file_id, token, Some(container)).unique()
                } else {
                    let chain = chains.chain_for(db, container);
                    sema.nameres_ident_in_scopes_at(hir_file_id, token, NameContext::Value, &chain)
                        .map(DefinitionClass::Definition)
                        .unique()
                };
                let full =
                    DefinitionClass::resolve_in(db, &context, hir_file_id, token, Some(container)).unique();
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
        let index = source_root_reference_index_for_root(host.raw_db(), SourceRootId(0));

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
        let group = |name: &str, def_marker: &str| {
            let def_range = def_range(def_marker);
            index
                .reference_groups_named(name)
                .into_iter()
                .find(|group| {
                    group
                        .definition_ranges
                        .iter()
                        .any(|range| range.file_id == file_id && range.range == def_range)
                })
                .unwrap_or_else(|| panic!("missing group {name} at {def_marker}"))
        };
        let reference =
            |group: &SemanticReferenceGroup, range: TextRange| -> (TextRange, ReferenceContext) {
                let reference = group
                    .references
                    .iter()
                    .find(|reference| reference.range == range)
                    .unwrap_or_else(|| panic!("missing reference at {range:?}"));
                (range, reference.context.clone())
            };

        // Same-name connection `.a(a)`: the name token pairs the local def,
        // the data token pairs the port def, both share the collapse range.
        let same_name_range = conn_name_at("same_name");
        let same_name_data_range = range_at("same_name_data");
        let collapse =
            TextRange::new(same_name_range.start(), same_name_data_range.end() + TextSize::of(")"));
        let child_a = group("a", "child_a");
        let top_a = group("a", "local_a");
        let name_ref = reference(child_a, conn_name_at("same_name"));
        let ReferenceContext::ConnName { ident_range, collapse_range, shorthand, side, paired } =
            &name_ref.1
        else {
            panic!("same-name name token should be ConnName: {:?}", name_ref.1);
        };
        assert_eq!(ident_range, &Some(same_name_data_range));
        assert_eq!(collapse_range, &Some(collapse));
        assert!(!shorthand);
        assert_eq!(side, &ConnSide::Port);
        let paired = paired.as_ref().expect("same-name conn should pair the local def");
        assert!(
            index
                .references_for_definition(*paired)
                .expect("paired def should have a group")
                .definition_ranges
                .iter()
                .any(|range| range.file_id == file_id && range.range == def_range("local_a")),
            "paired local def should be top.a"
        );
        let data_ref = reference(top_a, range_at("same_name_data"));
        let ReferenceContext::ConnData { name_range, collapse_range, paired } = &data_ref.1 else {
            panic!("same-name data token should be ConnData: {:?}", data_ref.1);
        };
        assert_eq!(name_range, &same_name_range);
        assert_eq!(collapse_range, &Some(collapse));
        let paired = paired.as_ref().expect("same-name conn should pair the port def");
        assert!(
            index
                .references_for_definition(*paired)
                .expect("paired def should have a group")
                .definition_ranges
                .iter()
                .any(|range| range.file_id == file_id && range.range == def_range("child_a")),
            "paired port def should be child.a"
        );

        // Non-same-name connection `.b(c)`: shape is recorded, no pairing.
        let child_b = group("b", "child_b");
        let name_ref = reference(child_b, conn_name_at("other_name"));
        let ReferenceContext::ConnName { ident_range, paired, .. } = &name_ref.1 else {
            panic!("non-same-name name token should be ConnName: {:?}", name_ref.1);
        };
        assert_eq!(ident_range, &Some(range_at("other_data")));
        assert_eq!(paired, &None);
        let top_c = group("c", "local_c");
        let data_ref = reference(top_c, range_at("other_data"));
        let ReferenceContext::ConnData { name_range, paired, .. } = &data_ref.1 else {
            panic!("non-same-name data token should be ConnData: {:?}", data_ref.1);
        };
        assert_eq!(name_range, &conn_name_at("other_name"));
        assert_eq!(paired, &None);

        // Shorthand `.b`: one reference in each side's group.
        let top_b = group("b", "local_b");
        let port_ref = reference(child_b, conn_name_at("shorthand"));
        let ReferenceContext::ConnName { collapse_range, shorthand, side, paired, .. } =
            &port_ref.1
        else {
            panic!("shorthand port reference should be ConnName: {:?}", port_ref.1);
        };
        assert!(shorthand);
        assert_eq!(collapse_range, &None);
        assert_eq!(side, &ConnSide::Port);
        let paired = paired.as_ref().expect("shorthand should pair the local def");
        assert!(
            index
                .references_for_definition(*paired)
                .expect("paired def should have a group")
                .definition_ranges
                .iter()
                .any(|range| range.file_id == file_id && range.range == def_range("local_b")),
            "shorthand port side should pair top.b"
        );
        let local_ref = reference(top_b, conn_name_at("shorthand"));
        let ReferenceContext::ConnName { side, paired, .. } = &local_ref.1 else {
            panic!("shorthand local reference should be ConnName: {:?}", local_ref.1);
        };
        assert_eq!(side, &ConnSide::Local);
        let paired = paired.as_ref().expect("shorthand should pair the port def");
        assert!(
            index
                .references_for_definition(*paired)
                .expect("paired def should have a group")
                .definition_ranges
                .iter()
                .any(|range| range.file_id == file_id && range.range == def_range("child_b")),
            "shorthand local side should pair child.b"
        );

        // Plain references stay Plain.
        let plain = reference(top_c, range_at("plain"));
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
        let db = host.raw_db();
        let tree = db.parse(HirFileId::from(file_id));
        let root = tree.root();
        let emitted = emit_token_index(root);
        for marker in ["param", "body"] {
            let target = resolve_semantic_target_with_emitted(
                db,
                file_id,
                markers[marker],
                Some(root),
                token_precedence,
                Some(&emitted),
            )
            .unique_for_intent(TargetIntent::FindReferences);
            assert!(
                matches!(target, Some(SemanticTarget::PreprocMacro(_))),
                "{marker} must remain owned by the preprocessor: {target:?}"
            );
        }
        let index = source_root_reference_index_for_root(host.raw_db(), SourceRootId(0));
        let definition_range = TextRange::new(markers["def"], markers["def"] + TextSize::of("x"));
        let preproc_ranges = [
            TextRange::new(markers["param"], markers["param"] + TextSize::of("x")),
            TextRange::new(markers["body"], markers["body"] + TextSize::of("x")),
        ];
        let group = index
            .reference_groups_named("x")
            .into_iter()
            .find(|group| {
                group
                    .definition_ranges
                    .iter()
                    .any(|range| range.file_id == file_id && range.range == definition_range)
            })
            .expect("the HDL declaration should have a semantic reference group");

        assert!(
            group
                .references
                .iter()
                .all(|reference| { !preproc_ranges.iter().any(|range| range == &reference.range) }),
            "preprocessor-owned x tokens must not become HDL references: {:?}",
            group.references
        );
        assert!(group.references.iter().any(|reference| {
            reference.range
                == TextRange::new(markers["ordinary"], markers["ordinary"] + TextSize::of("x"))
        }));
        assert!(group.references.iter().any(|reference| {
            reference.range == TextRange::new(markers["arg"], markers["arg"] + TextSize::of("x"))
        }));
    }
}
