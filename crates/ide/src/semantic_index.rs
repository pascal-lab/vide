use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootId,
    },
    container::InFile,
    db::HirDb,
    def_id::ModuleDefId,
    file::HirFileId,
    hir_def::{Ident, module::ModuleId},
    semantics::Semantics,
    source_map::IsSrc,
    symbol::DefId,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxElement, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, WalkEvent,
    has_text_range::HasTextRange, ptr::SyntaxTokenPtr, token::TokenKindExt,
};
use utils::{get::Get, line_index::TextRange};
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
    references::ReferenceCategory,
    semantic_target::{SemanticTarget, TargetIntent, resolve_semantic_target},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticIndex {
    references_by_definition: FxHashMap<ModuleDefId, SemanticReferenceGroup>,
    incoming_module_edges: FxHashMap<ModuleId, Box<[ModuleCallEdge]>>,
    outgoing_module_edges: FxHashMap<ModuleId, Box<[ModuleCallEdge]>>,
}

#[derive(Debug, Default)]
struct SemanticIndexBuilder {
    references_by_definition: FxHashMap<ModuleDefId, SemanticReferenceGroupBuilder>,
    incoming_module_edges: FxHashMap<ModuleId, Vec<ModuleCallEdge>>,
    outgoing_module_edges: FxHashMap<ModuleId, Vec<ModuleCallEdge>>,
}

#[derive(Debug)]
struct SemanticReferenceGroupBuilder {
    name: String,
    definition_ranges: Vec<SemanticDefinitionRange>,
    references: Vec<SemanticReference>,
}

impl ModuleIndex {
    pub(crate) fn for_source_root(
        db: &dyn WorkspaceSymbolIndexDb,
        source_root_id: SourceRootId,
    ) -> Self {
        let source_root = db.source_root(source_root_id);
        let mut modules_by_name: FxHashMap<Ident, Vec<SemanticModuleDefinition>> =
            FxHashMap::default();

        for file_id in source_root.iter() {
            let hir_file_id = HirFileId::from(file_id);
            for (_, defs) in db.file_scope(hir_file_id).iter_listing() {
                for module_id in defs
                    .iter()
                    .filter(|def_id| def_id.kind(db).is_instantiable_def())
                    .filter_map(|def_id| def_id.as_module(db))
                {
                    let Some(module) = SemanticModuleDefinition::new(db, module_id) else {
                        continue;
                    };
                    modules_by_name.entry(module.name.clone()).or_default().push(module);
                }
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
    fn new(db: &dyn HirDb, module_id: ModuleId) -> Option<Self> {
        let origin = DefId::new(db, module_id);
        let name = origin.name(db)?;
        let InFile { file_id, value: name_range } = origin.name_range(db)?;
        let InFile { value: full_range, .. } = origin.range(db)?;

        Some(Self { module_id, file_id: file_id.file_id(), name, name_range, full_range })
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
    pub(crate) fn for_source_root(db: &RootDb, source_root_id: SourceRootId) -> Self {
        let source_root = db.source_root(source_root_id);
        let module_index = source_root_module_index_for_root(db, source_root_id);
        let mut builder = SemanticIndexBuilder::default();

        builder.collect_module_edges(db, &module_index);
        for file_id in source_root.iter() {
            builder.collect_file(db, file_id);
        }

        builder.finish()
    }

    pub(crate) fn references_for_definition(
        &self,
        definition: ModuleDefId,
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

impl SemanticIndexBuilder {
    fn collect_file(&mut self, db: &RootDb, file_id: FileId) {
        let sema = Semantics::new(db);
        let parsed_file = sema.parse_file(file_id);
        let Some(root) = parsed_file.root() else {
            return;
        };
        let hir_file_id = HirFileId::from(file_id);

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
            let Some(SemanticTarget::Source(target)) =
                resolve_semantic_target(db, file_id, range.start(), Some(root), token_precedence)
                    .unique_for_intent(TargetIntent::FindReferences)
            else {
                continue;
            };

            for token in target.into_tokens().into_iter().filter(|token| token.kind().name_like()) {
                self.collect_token(db, &sema, hir_file_id, token);
            }
        }
    }

    fn collect_token(
        &mut self,
        db: &RootDb,
        sema: &Semantics<'_, RootDb>,
        file_id: HirFileId,
        token: SyntaxTokenWithParent<'_>,
    ) {
        let Some(range) = token.text_range() else {
            return;
        };
        let Some(class) = DefinitionClass::resolve(sema, file_id, token) else {
            return;
        };

        match class {
            DefinitionClass::Definition(definition) => {
                self.collect_definition_token(db, definition, file_id.file_id(), range, token)
            }
            DefinitionClass::PortConnShorthand { port, local } => {
                self.collect_definition_token(db, port, file_id.file_id(), range, token);
                self.collect_definition_token(db, local, file_id.file_id(), range, token);
            }
            DefinitionClass::Ambiguous(_) => {}
        }
    }

    fn collect_definition_token(
        &mut self,
        db: &RootDb,
        definition: ModuleDefId,
        file_id: FileId,
        range: TextRange,
        token: SyntaxTokenWithParent<'_>,
    ) {
        let origins = definition.origins(db);
        let Some(name) = origins.iter().find_map(|origin| origin.name(db)) else {
            return;
        };
        let definition_ranges = origins
            .iter()
            .filter_map(|origin| origin.name_range(db))
            .map(|InFile { file_id, value }| SemanticDefinitionRange {
                file_id: file_id.file_id(),
                range: value,
            })
            .unique()
            .collect_vec();
        let is_definition_site = definition_ranges.iter().any(|definition_range| {
            definition_range.file_id == file_id && definition_range.range == range
        });

        let group = self.references_by_definition.entry(definition).or_insert_with(|| {
            SemanticReferenceGroupBuilder {
                name: name.to_string(),
                definition_ranges,
                references: Vec::new(),
            }
        });

        if is_definition_site {
            return;
        }

        let reference = SemanticReference {
            file_id,
            range,
            category: ReferenceCategory::from_tok(token),
            ptr: SyntaxTokenPtr::from_token(token),
        };
        if !group.references.iter().any(|existing| {
            existing.file_id == reference.file_id && existing.range == reference.range
        }) {
            group.references.push(reference);
        }
    }

    fn collect_module_edges(&mut self, db: &RootDb, module_index: &ModuleIndex) {
        for caller in module_index.all_module_definitions() {
            let (module, source_map) = db.module_with_source_map(caller.module_id);
            for (instantiation_id, instantiation) in module.instantiations.iter() {
                let Some(callee_module_id) =
                    resolve_hir_instantiation_target(db, caller.file_id, instantiation)
                else {
                    continue;
                };
                let Some(callee) = SemanticModuleDefinition::new(db, callee_module_id) else {
                    continue;
                };
                let Some(src) = source_map.get(instantiation_id) else {
                    continue;
                };
                let Some(call_range) = instantiation_name_range(db, caller.file_id, src) else {
                    continue;
                };

                self.collect_module_edge(
                    caller.module_id,
                    callee.module_id,
                    ModuleCallEdge {
                        caller: caller.call_item(),
                        callee: callee.call_item(),
                        call_range,
                    },
                );
            }
        }
    }

    fn collect_module_edge(
        &mut self,
        caller_module_id: ModuleId,
        callee_module_id: ModuleId,
        edge: ModuleCallEdge,
    ) {
        push_unique_edge(
            self.outgoing_module_edges.entry(caller_module_id).or_default(),
            edge.clone(),
        );
        push_unique_edge(self.incoming_module_edges.entry(callee_module_id).or_default(), edge);
    }

    fn finish(self) -> SemanticIndex {
        SemanticIndex {
            references_by_definition: self
                .references_by_definition
                .into_iter()
                .map(|(key, group)| (key, group.finish()))
                .collect(),
            incoming_module_edges: finish_edge_map(self.incoming_module_edges),
            outgoing_module_edges: finish_edge_map(self.outgoing_module_edges),
        }
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
    db: &RootDb,
    file_id: FileId,
    src: hir::hir_def::module::instantiation::InstantiationSrc,
) -> Option<TextRange> {
    let tree = db.parse_src(file_id);
    let root = tree.root()?;
    let instantiation_range = src.range();
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
