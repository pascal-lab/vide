use base_db::{
    source_db::{SourceDb, SourceRootDb},
    source_root::SourceRootId,
};
use hir_def::{Ident, container::InFile, def_id::DefId, module::ModuleId, symbol::DefOrigin};
use hir_ty::db::TyDb;
use itertools::Itertools;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxElement, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, WalkEvent,
    has_text_range::HasTextRange, ptr::SyntaxTokenPtr, token::TokenKindExt,
};
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{
    db::{
        root_db::RootDb,
        workspace_symbol_index_db::{
            WorkspaceSymbolIndexDb, source_root_module_index_for_root,
            source_root_semantic_index_for_root,
        },
    },
    definitions::DefinitionClass,
    module_resolution::resolve_hir_instantiation_target,
    navigation_target::nav_location,
    references::{ReferenceCategory, search::resolve_source_range},
    semantic_target::{SemanticTarget, TargetIntent, resolve_semantic_target_with_emitted},
    source_targets::preproc::emit_token_index,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SemanticDefinitionRange {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticReference {
    pub file_id: FileId,
    pub range: TextRange,
    pub category: ReferenceCategory,
    pub ptr: SyntaxTokenPtr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticReferenceGroup {
    pub name: String,
    pub definition_ranges: Box<[SemanticDefinitionRange]>,
    pub references: Box<[SemanticReference]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticModuleDefinition {
    pub module_id: ModuleId,
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
pub struct SemanticIndex {
    references_by_definition: FxHashMap<DefId, SemanticReferenceGroup>,
    incoming_module_edges: FxHashMap<ModuleId, Box<[ModuleCallEdge]>>,
    outgoing_module_edges: FxHashMap<ModuleId, Box<[ModuleCallEdge]>>,
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
    edges: Vec<(ModuleId, ModuleId, ModuleCallEdge)>,
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

        for file_id in source_root.iter() {
            for module in db.file_module_index(file_id).modules.iter() {
                modules_by_name.entry(module.name.clone()).or_default().push(module.clone());
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
    fn new(db: &dyn TyDb, module_id: ModuleId) -> Option<Self> {
        let origin = DefOrigin::new(db, module_id);
        let name = origin.name(db)?;
        let InFile { file_id, value: name_range } = origin.name_range(db)?;
        let InFile { value: full_range, .. } = origin.range(db)?;
        let (file_id, name_range, full_range) =
            nav_location(db, file_id, Some(name_range), full_range)?;

        Some(Self {
            module_id,
            file_id,
            name,
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

impl SemanticIndex {
    /// Merges the per-file semantic indexes and module edges of a source root.
    ///
    /// The merge is pure memory assembly: no name resolution happens here, so
    /// a change in one file only re-runs that file's index and this pass.
    pub(crate) fn for_source_root(
        db: &dyn WorkspaceSymbolIndexDb,
        source_root_id: SourceRootId,
    ) -> Self {
        let source_root = db.source_root(source_root_id);
        let mut references_by_definition: FxHashMap<DefId, SemanticReferenceGroupBuilder> =
            FxHashMap::default();
        let mut incoming_module_edges: FxHashMap<ModuleId, Vec<ModuleCallEdge>> =
            FxHashMap::default();
        let mut outgoing_module_edges: FxHashMap<ModuleId, Vec<ModuleCallEdge>> =
            FxHashMap::default();

        for file_id in source_root.iter() {
            db.unwind_if_cancelled();
            let file_index = db.file_semantic_index(file_id);
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
            for (caller, callee, edge) in &db.file_module_edges(file_id).edges {
                push_unique_edge(outgoing_module_edges.entry(*caller).or_default(), edge.clone());
                push_unique_edge(incoming_module_edges.entry(*callee).or_default(), edge.clone());
            }
        }

        SemanticIndex {
            references_by_definition: references_by_definition
                .into_iter()
                .map(|(key, group)| (key, group.finish()))
                .collect(),
            incoming_module_edges: finish_edge_map(incoming_module_edges),
            outgoing_module_edges: finish_edge_map(outgoing_module_edges),
        }
    }

    pub(crate) fn references_for_definition(
        &self,
        definition: DefId,
    ) -> Option<&SemanticReferenceGroup> {
        self.references_by_definition.get(&definition)
    }

    pub(crate) fn incoming_module_edges(&self, module_id: ModuleId) -> &[ModuleCallEdge] {
        self.incoming_module_edges.get(&module_id).map_or(&[], |edges| edges.as_ref())
    }

    pub(crate) fn outgoing_module_edges(&self, module_id: ModuleId) -> &[ModuleCallEdge] {
        self.outgoing_module_edges.get(&module_id).map_or(&[], |edges| edges.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn reference_groups_named(&self, name: &str) -> Vec<&SemanticReferenceGroup> {
        self.references_by_definition.values().filter(|group| group.name == name).collect()
    }
}

impl FileSemanticIndex {
    pub(crate) fn references_for_definition(
        &self,
        definition: DefId,
    ) -> Option<&FileReferenceGroup> {
        self.groups.get(&definition)
    }

    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let tree = db.parse(file_id.into());
        let Some(root) = tree.root() else {
            return Self::default();
        };
        let hir_file_id = HirFileId::from(file_id);

        // Macro-emitted tokens share the call-site display range, so resolving
        // them needs a tree walk indexed by trace id. Build it once per file
        // and share it across every token resolution instead of re-walking the
        // tree for each macro-region token.
        let has_backtick = db.file_text(file_id).contains('`');
        let emitted_index = has_backtick.then(|| emit_token_index(root));

        let mut groups: FxHashMap<DefId, FileReferenceGroup> = FxHashMap::default();
        for event in root.elem_preorder() {
            let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
                continue;
            };
            if !token.kind().name_like() {
                continue;
            }
            let Some(range) = token.text_range() else {
                continue;
            };
            let Some(SemanticTarget::Source(target)) = resolve_semantic_target_with_emitted(
                db,
                file_id,
                range.start(),
                Some(root),
                token_precedence,
                emitted_index.as_ref(),
            )
            .unique_for_intent(TargetIntent::FindReferences) else {
                continue;
            };

            for token in target.into_tokens().into_iter().filter(|token| token.kind().name_like()) {
                collect_token(db, hir_file_id, token, &mut groups);
            }
        }
        Self { groups }
    }
}

pub(crate) fn file_semantic_index_query(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
) -> Arc<FileSemanticIndex> {
    Arc::new(FileSemanticIndex::for_file(db, file_id))
}

fn collect_token(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: HirFileId,
    token: SyntaxTokenWithParent<'_>,
    groups: &mut FxHashMap<DefId, FileReferenceGroup>,
) {
    let Some(range) = token.text_range() else {
        return;
    };
    let Some(class) = DefinitionClass::resolve(db, file_id, token).unique() else {
        return;
    };

    match class {
        DefinitionClass::Definition(definition) => {
            collect_definition_token(db, definition, file_id.expect_file(), range, token, groups)
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            collect_definition_token(db, port, file_id.expect_file(), range, token, groups);
            collect_definition_token(db, local, file_id.expect_file(), range, token, groups);
        }
    }
}

fn collect_definition_token(
    db: &dyn WorkspaceSymbolIndexDb,
    definition: DefId,
    file_id: FileId,
    range: TextRange,
    token: SyntaxTokenWithParent<'_>,
    groups: &mut FxHashMap<DefId, FileReferenceGroup>,
) {
    let origins = definition.origins(db);
    let Some(name) = origins.iter().find_map(|origin| origin.name(db)) else {
        return;
    };
    let definition_ranges = definition_ranges_for(db, definition);
    let is_definition_site = definition_ranges.iter().any(|definition_range| {
        definition_range.file_id == file_id && definition_range.range == range
    });
    if is_definition_site {
        return;
    }

    let group = groups
        .entry(definition)
        .or_insert_with(|| FileReferenceGroup { name: name.to_string(), references: Vec::new() });
    let reference = SemanticReference {
        file_id,
        range,
        category: ReferenceCategory::from_tok(token),
        ptr: SyntaxTokenPtr::from_token(token),
    };
    if !group
        .references
        .iter()
        .any(|existing| existing.file_id == reference.file_id && existing.range == reference.range)
    {
        group.references.push(reference);
    }
}

/// Definition name ranges of `definition` mapped to user-facing files, in
/// origin order. Computed once per definition at merge time.
fn definition_ranges_for(
    db: &dyn WorkspaceSymbolIndexDb,
    definition: DefId,
) -> Vec<SemanticDefinitionRange> {
    definition
        .origins(db)
        .iter()
        .filter_map(|origin| {
            let InFile { file_id, value } = origin.name_range(db)?;
            let (file_id, range) = resolve_source_range(db, file_id, value)?;
            Some(SemanticDefinitionRange { file_id, range })
        })
        .unique()
        .collect_vec()
}

impl FileModuleIndex {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let mut modules = Vec::new();
        for (_, defs) in db.file_scope(hir_file_id).iter_listing() {
            for module_id in defs
                .iter()
                .filter(|def_id| def_id.kind(db).is_instantiable_def())
                .filter_map(|def_id| def_id.primary_origin(db).as_module(db))
            {
                let Some(module) = SemanticModuleDefinition::new(db, module_id) else {
                    continue;
                };
                modules.push(module);
            }
        }
        Self { modules }
    }
}

pub(crate) fn file_module_index_query(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
) -> Arc<FileModuleIndex> {
    Arc::new(FileModuleIndex::for_file(db, file_id))
}

impl FileModuleEdges {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let mut edges = Vec::new();
        for (_, defs) in db.file_scope(hir_file_id).iter_listing() {
            for def_id in defs.iter().filter(|def_id| def_id.kind(db).is_instantiable_def()) {
                let Some(caller) = def_id.primary_origin(db).as_module(db) else {
                    continue;
                };
                let Some(caller_def) = SemanticModuleDefinition::new(db, caller) else {
                    continue;
                };
                let module = db.module_with_source_map(caller);
                for (instantiation_id, instantiation) in module.instantiations.iter() {
                    let Some(callee_module_id) =
                        resolve_hir_instantiation_target(db, file_id, instantiation)
                    else {
                        continue;
                    };
                    let Some(callee) = SemanticModuleDefinition::new(db, callee_module_id) else {
                        continue;
                    };
                    let Some(call_range) = module
                        .source_range(instantiation_id)
                        .and_then(|range| instantiation_name_range(db, file_id, range))
                    else {
                        continue;
                    };
                    edges.push((
                        caller,
                        callee.module_id,
                        ModuleCallEdge {
                            caller: caller_def.call_item(),
                            callee: callee.call_item(),
                            call_range,
                        },
                    ));
                }
            }
        }
        Self { edges }
    }
}

pub(crate) fn file_module_edges_query(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
) -> Arc<FileModuleEdges> {
    Arc::new(FileModuleEdges::for_file(db, file_id))
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
    edges_for_index: impl Fn(&SemanticIndex, ModuleId) -> &[ModuleCallEdge],
) -> Vec<ModuleCallEdge> {
    let Some(module_id) = module_id_at_range(db, file_id, name_range) else {
        return Vec::new();
    };

    let mut source_root_ids =
        db.files().iter().map(|&file_id| db.source_root_id(file_id)).collect::<Vec<_>>();
    source_root_ids.sort_unstable();
    source_root_ids.dedup();

    let mut edges = Vec::new();
    for source_root_id in source_root_ids {
        let index = source_root_semantic_index_for_root(db, source_root_id);
        edges.extend(edges_for_index(&index, module_id).iter().cloned());
    }
    sort_and_dedup_edges(&mut edges);
    edges
}

fn module_id_at_range(db: &RootDb, file_id: FileId, name_range: TextRange) -> Option<ModuleId> {
    let module_index = source_root_module_index_for_root(db, db.source_root_id(file_id));
    module_index.module_definition_at(file_id, name_range).map(|module| module.module_id)
}

fn instantiation_name_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    instantiation_range: TextRange,
) -> Option<TextRange> {
    let tree = db.parse_src_for_compilation(file_id);
    let root = tree.root()?;
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
    edges_by_module: FxHashMap<ModuleId, Vec<ModuleCallEdge>>,
) -> FxHashMap<ModuleId, Box<[ModuleCallEdge]>> {
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
    usize::from(kind.name_like())
}
