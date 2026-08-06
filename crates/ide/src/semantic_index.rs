use base_db::{source_db::SourceRootDb, source_root::SourceRootId};
use hir_def::{
    Ident,
    container::{ArenaOwnerId, InFile, ScopeParent},
    def_id::DefId,
    module::{
        ModuleId,
        generate::{GenerateBlockId, GenerateBlockLoc, GenerateBlockSrc},
    },
    pathres::ResolvedScopes,
    symbol::{DefOrigin, NameContext},
};
use hir_semantics::semantics::SemanticsImpl;
use hir_ty::db::TyDb;
use itertools::Itertools;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    SyntaxAncestors, SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken, SyntaxTokenWithParent,
    TokenKind, WalkEvent,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
    ptr::SyntaxTokenPtr,
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
            source_root_semantic_index_for_root,
        },
    },
    definitions::{DefinitionClass, rightmost_name_token},
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
        let origin = DefOrigin::new(module_id);
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
            db.unwind_if_revision_cancelled();
            let file_index = db.file_semantic_index(file_id);
            for (definition, group) in &file_index.groups {
                let builder =
                    references_by_definition.entry(definition.clone()).or_insert_with(|| {
                        SemanticReferenceGroupBuilder {
                            name: group.name.clone(),
                            definition_ranges: definition_ranges_for(db, definition.clone()),
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

        let sema = SemanticsImpl::new(db);
        let mut containers = ContainerCache::new();
        let mut chains = ScopeChainCache::new();
        let mut groups: FxHashMap<DefId, FileReferenceGroup> = FxHashMap::default();
        let mut trace = IndexBuildTrace::start();
        // Port definitions of named port connections by name token range,
        // populated when the name token resolves (it precedes the data token
        // in source order) and read back when the data token is collected.
        let mut conn_port_by_name = FxHashMap::default();
        let text = db.file_text(file_id);
        for event in root.elem_preorder() {
            match event {
                WalkEvent::Enter(SyntaxElement::Node(node)) => {
                    trace.count_special_kinds(&node);
                }
                WalkEvent::Leave(SyntaxElement::Node(_)) => {}
                WalkEvent::Enter(SyntaxElement::Token(token)) => {
                    if !token.kind().name_like() {
                        continue;
                    }
                    trace.tokens += 1;
                    let (range_cost, range) = timed(|| token.text_range());
                    trace.range += range_cost;
                    let Some(range) = range else {
                        continue;
                    };
                    // Preserve the semantic target's preprocessor ownership
                    // checks while reusing the emitted-token index for macro
                    // expansion tokens. Preprocessor definitions, parameters,
                    // and includes are indexed by their own indexes rather
                    // than as HDL references.
                    let (target_cost, target) = timed(|| {
                        resolve_semantic_target_with_emitted(
                            db,
                            file_id,
                            range.start(),
                            Some(root),
                            token_precedence,
                            emitted_index.as_ref(),
                        )
                        .unique_for_intent(TargetIntent::FindReferences)
                    });
                    trace.source_target += target_cost;
                    let Some(SemanticTarget::Source(target)) = target else {
                        continue;
                    };

                    let (container_cost, container) =
                        timed(|| containers.container_for(&sema, hir_file_id, token.parent));
                    trace.container += container_cost;
                    for token in
                        target.into_tokens().into_iter().filter(|token| token.kind().name_like())
                    {
                        // The heuristic chain in `DefinitionClass::resolve_in`
                        // can only diverge from plain value-name resolution at
                        // the token positions tested by `token_in_special_context`;
                        // every other token resolves as a plain value identifier.
                        let in_special_context = token_in_special_context(token);
                        if in_special_context {
                            trace.special_tokens += 1;
                        }
                        let (collect_cost, ()) = timed(|| {
                            collect_token(
                                db,
                                hir_file_id,
                                token,
                                container.clone(),
                                in_special_context,
                                &mut chains,
                                &mut conn_port_by_name,
                                &text,
                                &mut groups,
                                &mut trace,
                            )
                        });
                        trace.collect += collect_cost;
                    }
                }
                WalkEvent::Leave(SyntaxElement::Token(_)) => {}
            }
        }
        trace.report(file_id);
        Self { groups }
    }
}

/// Set when `VIDE_INDEX_BUILD_TRACE` is set.
struct IndexBuildTrace {
    enabled: bool,
    range: std::time::Duration,
    source_target: std::time::Duration,
    container: std::time::Duration,
    collect: std::time::Duration,
    resolve: std::time::Duration,
    resolve_fast: std::time::Duration,
    resolve_slow: std::time::Duration,
    chain_ns: u64,
    nameres_ns: u64,
    definition: std::time::Duration,
    total: std::time::Instant,
    tokens: usize,
    special_tokens: usize,
    kind_hits: [usize; 10],
}

impl IndexBuildTrace {
    fn start() -> Self {
        Self {
            enabled: std::env::var_os("VIDE_INDEX_BUILD_TRACE").is_some(),
            range: std::time::Duration::ZERO,
            source_target: std::time::Duration::ZERO,
            container: std::time::Duration::ZERO,
            collect: std::time::Duration::ZERO,
            resolve: std::time::Duration::ZERO,
            resolve_fast: std::time::Duration::ZERO,
            resolve_slow: std::time::Duration::ZERO,
            chain_ns: 0,
            nameres_ns: 0,
            definition: std::time::Duration::ZERO,
            total: std::time::Instant::now(),
            tokens: 0,
            special_tokens: 0,
            kind_hits: [0; 10],
        }
    }

    fn record_chain(&mut self, chain: std::time::Duration, nameres: std::time::Duration) {
        self.chain_ns += chain.as_nanos() as u64;
        self.nameres_ns += nameres.as_nanos() as u64;
    }

    fn count_special_kinds(&mut self, node: &SyntaxNode<'_>) {
        if !self.enabled {
            return;
        }
        let kind = node.kind();
        self.kind_hits[0] += usize::from(ast::MemberAccessExpression::can_cast(kind));
        self.kind_hits[1] += usize::from(ast::ScopedName::can_cast(kind));
        self.kind_hits[2] += usize::from(ast::ModuleDeclaration::can_cast(kind));
        self.kind_hits[3] += usize::from(ast::PrimitiveInstantiation::can_cast(kind));
        self.kind_hits[4] += usize::from(ast::CheckerInstantiation::can_cast(kind));
        self.kind_hits[5] += usize::from(ast::HierarchyInstantiation::can_cast(kind));
        self.kind_hits[6] += usize::from(ast::PackageImportItem::can_cast(kind));
        self.kind_hits[7] += usize::from(ast::NamedParamAssignment::can_cast(kind));
        self.kind_hits[8] += usize::from(ast::NamedPortConnection::can_cast(kind));
        self.kind_hits[9] += usize::from(ast::NamedType::can_cast(kind));
    }

    fn report(&self, file_id: FileId) {
        if !self.enabled {
            return;
        }
        eprintln!(
            "[index trace] file={file_id:?} tokens={} special={} total={:?}\n  range={:?} source_target={:?} container={:?}\n  collect={:?} (resolve={:?} [fast={:?} slow={:?}] chain={:?} nameres={:?} definition={:?})\n  kind_hits={:?}",
            self.tokens,
            self.special_tokens,
            self.total.elapsed(),
            self.range,
            self.source_target,
            self.container,
            self.collect,
            self.resolve,
            self.resolve_fast,
            self.resolve_slow,
            std::time::Duration::from_nanos(self.chain_ns),
            std::time::Duration::from_nanos(self.nameres_ns),
            self.definition,
            self.kind_hits,
        );
    }
}

fn timed<T>(f: impl FnOnce() -> T) -> (std::time::Duration, T) {
    let start = std::time::Instant::now();
    let value = f();
    (start.elapsed(), value)
}

/// Caches HIR container ids by syntax node while walking a tree.
///
/// `source_to_def::find_container` finds a token's container by walking up
/// the ancestor chain and matching every node; doing that per token makes
/// the index build pay the ancestor walk for every name-like token. This
/// cache keeps the same walk shape (up to the nearest container node, then
/// a lookup), but computes each container id once instead of once per token.
///
/// The node dispatch must stay in sync with
/// `hir_semantics::semantics::source_to_def::container_to_def`: the
/// module/block/subroutine arms use the public `Semantics` projections, and
/// generate blocks / single-member generate branches intern through
/// `intern_generate_block` with the nearest enclosing container as parent.
///
/// The key is the Slang node itself, not `SyntaxNodePtr`: macro-emitted nodes
/// can share a display range and kind at their call site, while their pointer
/// identities remain distinct.
struct ContainerCache<'tree> {
    by_node: FxHashMap<SyntaxNode<'tree>, ArenaOwnerId>,
}

impl<'tree> ContainerCache<'tree> {
    fn new() -> Self {
        Self { by_node: FxHashMap::default() }
    }

    /// The container of a token: the nearest container node on its ancestor
    /// chain whose id computes successfully, mirroring
    /// `find_map(container_to_def)`; nodes that fail to lower are skipped.
    fn container_for(
        &mut self,
        sema: &SemanticsImpl<'_>,
        file_id: HirFileId,
        token_parent: SyntaxNode<'tree>,
    ) -> ArenaOwnerId {
        for node in SyntaxAncestors::start_from(token_parent) {
            if is_container_node(&node)
                && let Some(id) = self.try_id_for(sema, file_id, node)
            {
                return id;
            }
        }
        file_id.into()
    }

    /// The container of a node's subtree: like
    /// [`container_for`](Self::container_for) but starting above `node`.
    fn parent_of(
        &mut self,
        sema: &SemanticsImpl<'_>,
        file_id: HirFileId,
        node: SyntaxNode<'tree>,
    ) -> ArenaOwnerId {
        for ancestor in SyntaxAncestors::start_from(node).skip(1) {
            if is_container_node(&ancestor)
                && let Some(id) = self.try_id_for(sema, file_id, ancestor)
            {
                return id;
            }
        }
        file_id.into()
    }

    fn try_id_for(
        &mut self,
        sema: &SemanticsImpl<'_>,
        file_id: HirFileId,
        node: SyntaxNode<'tree>,
    ) -> Option<ArenaOwnerId> {
        if let Some(id) = self.by_node.get(&node) {
            return Some(id.clone());
        }
        let id = container_id_for_node(sema, file_id, node, self)?;
        self.by_node.insert(node, id.clone());
        Some(id)
    }
}

/// Resolved scope chains by container. The nameres fast path looks every
/// token up in its container's chain; resolving the chain once per container
/// avoids per-token salsa `scope_for` queries, whose memos revalidate against
/// every intervening query during the index build and recompute O(scope
/// size) on each miss.
struct ScopeChainCache {
    by_container: FxHashMap<ArenaOwnerId, Arc<ResolvedScopes>>,
}

impl ScopeChainCache {
    fn new() -> Self {
        Self { by_container: FxHashMap::default() }
    }

    fn chain_for(
        &mut self,
        db: &dyn WorkspaceSymbolIndexDb,
        container: ArenaOwnerId,
    ) -> Arc<ResolvedScopes> {
        if let Some(chain) = self.by_container.get(&container) {
            return chain.clone();
        }
        let chain = Arc::new({
            let scope_ids =
                ScopeParent::start_from(container.clone().into()).collect::<SmallVec<[_; 4]>>();
            let scopes = scope_ids.iter().map(|id| db.scope_for(id.clone())).collect::<Vec<_>>();
            let unit = db.unit_scope();
            ResolvedScopes { scope_ids, scopes, unit }
        });
        self.by_container.insert(container, chain.clone());
        chain
    }
}

/// Mirrors `source_to_def::container_to_def`'s node dispatch. Uses `cast`
/// (not `can_cast`) on every arm: slang's `can_cast` accepts sub-kind
/// relations (e.g. generate blocks pass `BlockStatement::can_cast`), which
/// would desynchronize enter/leave bookkeeping.
fn is_container_node(node: &SyntaxNode<'_>) -> bool {
    ast::ModuleDeclaration::cast(*node).is_some()
        || ast::BlockStatement::cast(*node).is_some()
        || ast::FunctionDeclaration::cast(*node).is_some()
        || ast::CompilationUnit::cast(*node).is_some()
        || ast::GenerateBlock::cast(*node).is_some()
        || (ast::Member::cast(*node).is_some() && is_generate_branch_member(*node))
}

fn container_id_for_node<'tree>(
    sema: &SemanticsImpl<'_>,
    file_id: HirFileId,
    node: SyntaxNode<'tree>,
    cache: &mut ContainerCache<'tree>,
) -> Option<ArenaOwnerId> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return sema.module_to_def(file_id, module).map(Into::into);
    }
    if let Some(block) = ast::BlockStatement::cast(node) {
        return sema.block_to_def(file_id, block).map(Into::into);
    }
    if let Some(func) = ast::FunctionDeclaration::cast(node) {
        return sema.subroutine_to_def(file_id, func).map(Into::into);
    }
    if ast::CompilationUnit::cast(node).is_some() {
        return Some(file_id.into());
    }
    if let Some(block) = ast::GenerateBlock::cast(node) {
        let src = GenerateBlockSrc::from_generate_block(block);
        let parent = cache.parent_of(sema, file_id, block.syntax());
        return Some(intern_generate_container(file_id, src, parent));
    }
    let member = ast::Member::cast(node)?;
    if !is_generate_branch_member(node) {
        return None;
    }
    let parent = cache.parent_of(sema, file_id, node);
    Some(intern_generate_container(file_id, GenerateBlockSrc::from(member), parent))
}

fn intern_generate_container(
    file_id: HirFileId,
    src: GenerateBlockSrc,
    parent: ArenaOwnerId,
) -> ArenaOwnerId {
    GenerateBlockId::new(GenerateBlockLoc { cont_id: parent, src: InFile::new(file_id, src) })
        .into()
}

/// Mirrors `source_to_def::is_generate_branch_member`: a member is a
/// single-member generate branch when it sits inside an if/case generate and
/// no stronger container (module, block, generate region) separates it.
/// The predicate itself lives in `hir-semantics`; only the container
/// dispatch is mirrored here.
fn is_generate_branch_member(member: SyntaxNode<'_>) -> bool {
    hir_semantics::semantics::is_generate_branch_member(member)
}

#[allow(clippy::too_many_arguments)]
fn collect_token(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: HirFileId,
    token: SyntaxTokenWithParent<'_>,
    container: ArenaOwnerId,
    in_special_context: bool,
    chains: &mut ScopeChainCache,
    conn_port_by_name: &mut FxHashMap<TextRange, DefId>,
    text: &str,
    groups: &mut FxHashMap<DefId, FileReferenceGroup>,
    trace: &mut IndexBuildTrace,
) {
    let Some(range) = token.text_range() else {
        return;
    };
    let (resolve_cost, class) = timed(|| {
        if in_special_context {
            let start = std::time::Instant::now();
            let class =
                DefinitionClass::resolve_in(db, file_id, token, Some(container.clone())).unique();
            trace.resolve_slow += start.elapsed();
            class
        } else {
            let start = std::time::Instant::now();
            // Fast path: outside every syntax context the heuristic chain in
            // `DefinitionClass::resolve` (member access, scoped names,
            // instantiations, package imports, named connections) is provably
            // empty, so resolve as a plain value identifier directly. The
            // scope chain is resolved once per container; per-token salsa
            // `scope_for` queries revalidate their memos against every
            // intervening query and recompute O(scope size) each time.
            let sema = SemanticsImpl::new(db);
            let chain_start = std::time::Instant::now();
            let chain = chains.chain_for(db, container.clone());
            let chain_cost = chain_start.elapsed();
            let class = sema
                .nameres_ident_in_scopes(token, NameContext::Value, &chain)
                .map(DefinitionClass::Definition)
                .unique();
            if trace.enabled {
                trace.record_chain(chain_cost, start.elapsed() - chain_cost);
            }
            trace.resolve_fast += start.elapsed();
            class
        }
    });
    trace.resolve += resolve_cost;
    let Some(class) = class else {
        return;
    };

    let (definition_cost, ()) = timed(|| match &class {
        DefinitionClass::Definition(definition) => {
            let context = reference_context(
                db,
                token,
                &class,
                container.clone(),
                chains,
                conn_port_by_name,
                text,
                ConnSide::Port,
            );
            collect_definition_token(
                db,
                definition.clone(),
                file_id.expect_file(),
                range,
                token,
                &context,
                groups,
            )
        }
        DefinitionClass::PortConnShorthand { port, local } => {
            let port_context = reference_context(
                db,
                token,
                &class,
                container.clone(),
                chains,
                conn_port_by_name,
                text,
                ConnSide::Port,
            );
            let local_context = reference_context(
                db,
                token,
                &class,
                container.clone(),
                chains,
                conn_port_by_name,
                text,
                ConnSide::Local,
            );
            collect_definition_token(
                db,
                port.clone(),
                file_id.expect_file(),
                range,
                token,
                &port_context,
                groups,
            );
            collect_definition_token(
                db,
                local.clone(),
                file_id.expect_file(),
                range,
                token,
                &local_context,
                groups,
            );
        }
    });
    trace.definition += definition_cost;
}

/// The role of a token inside a named port connection, if any, computed from
/// the token's syntax position alone.
enum ConnTokenRole<'tree> {
    /// The token is the `.name` of the connection.
    Name(ast::NamedPortConnection<'tree>),
    /// The token is a simple identifier in the data position.
    Data(ast::NamedPortConnection<'tree>),
}

fn conn_token_role<'tree>(token: SyntaxTokenWithParent<'tree>) -> Option<ConnTokenRole<'tree>> {
    let SyntaxTokenWithParent { parent, tok } = token;
    if let Some(conn) = ast::NamedPortConnection::cast(parent) {
        return conn.name().is_some_and(|name| name == tok).then_some(ConnTokenRole::Name(conn));
    }
    if ast::Name::can_cast(parent.kind()) {
        // The data identifier of a simple named port connection sits at a
        // fixed depth below the connection node (the wrapper expression
        // nodes are virtual).
        if let Some(node) = SyntaxAncestors::start_from(parent).nth(3)
            && let Some(conn) = ast::NamedPortConnection::cast(node)
            && conn_data_ident(conn).is_some_and(|ident| ident == tok)
        {
            return Some(ConnTokenRole::Data(conn));
        }
    }
    None
}

/// The identifier token of a connection's data side, when the data is a
/// simple identifier (bare name or empty select). Mirrors the extraction in
/// the rename edit rules.
fn conn_data_ident(conn: ast::NamedPortConnection<'_>) -> Option<SyntaxToken<'_>> {
    use ast::{Expression, Name};
    let expr = conn.expr()?.as_simple_property_expr()?.expr().as_simple_sequence_expr()?.expr();
    match expr {
        Expression::Name(Name::IdentifierName(ident)) => ident.identifier(),
        Expression::Name(Name::IdentifierSelectName(ident))
            if ident.selectors().children().next().is_none() =>
        {
            ident.identifier()
        }
        _ => None,
    }
}

struct ConnShape {
    name_range: TextRange,
    ident_range: Option<TextRange>,
    collapse_range: Option<TextRange>,
    shorthand: bool,
}

fn conn_shape(conn: ast::NamedPortConnection<'_>) -> Option<ConnShape> {
    let name_range = conn.name()?.text_range_in(conn.syntax())?;
    let collapse_range = conn
        .close_paren()
        .and_then(|token| token.text_range_in(conn.syntax()))
        .map(|range| TextRange::new(name_range.start(), range.end()));
    let ident_range = conn_data_ident(conn).and_then(|token| token.text_range_in(conn.syntax()));
    let shorthand = conn.open_paren().is_none() && conn.close_paren().is_none();
    Some(ConnShape { name_range, ident_range, collapse_range, shorthand })
}

fn range_text(text: &str, range: TextRange) -> &str {
    &text[usize::from(range.start())..usize::from(range.end())]
}

fn is_same_name_conn(text: &str, conn: &ConnShape) -> bool {
    conn.ident_range
        .is_some_and(|ident| range_text(text, conn.name_range) == range_text(text, ident))
}

/// The [`ReferenceContext`] of a token resolved to `class`. `side` selects
/// the shorthand side; non-shorthand tokens produce the same context for
/// either side.
#[allow(clippy::too_many_arguments)]
fn reference_context(
    db: &dyn WorkspaceSymbolIndexDb,
    token: SyntaxTokenWithParent<'_>,
    class: &DefinitionClass,
    container: ArenaOwnerId,
    chains: &mut ScopeChainCache,
    conn_port_by_name: &mut FxHashMap<TextRange, DefId>,
    text: &str,
    side: ConnSide,
) -> ReferenceContext {
    let Some(role) = conn_token_role(token) else {
        return ReferenceContext::Plain;
    };
    let sema = SemanticsImpl::new(db);
    match role {
        ConnTokenRole::Data(conn) => {
            let Some(shape) = conn_shape(conn) else {
                return ReferenceContext::Plain;
            };
            ReferenceContext::ConnData {
                name_range: shape.name_range,
                collapse_range: shape.collapse_range,
                paired: is_same_name_conn(text, &shape)
                    .then(|| conn_port_by_name.get(&shape.name_range).cloned())
                    .flatten(),
            }
        }
        ConnTokenRole::Name(conn) => {
            let Some(shape) = conn_shape(conn) else {
                return ReferenceContext::Plain;
            };
            if shape.shorthand {
                let (side, paired) = match class {
                    DefinitionClass::PortConnShorthand { port, local } => {
                        let paired = match side {
                            ConnSide::Port => Some(local.clone()),
                            ConnSide::Local => Some(port.clone()),
                        };
                        (side, paired)
                    }
                    DefinitionClass::Definition(def) => {
                        // One-sided shorthand resolution: the local side is the
                        // definition when plain value resolution matches it.
                        let chain = chains.chain_for(db, container.clone());
                        let is_local = sema
                            .nameres_ident_in_scopes(token, NameContext::Value, &chain)
                            .unique()
                            .is_some_and(|local| local == *def);
                        (if is_local { ConnSide::Local } else { ConnSide::Port }, None)
                    }
                };
                return ReferenceContext::ConnName {
                    ident_range: None,
                    collapse_range: None,
                    shorthand: true,
                    side,
                    paired,
                };
            }
            let same_name = is_same_name_conn(text, &shape);
            let paired = same_name
                .then(|| {
                    let chain = chains.chain_for(db, container.clone());
                    conn_data_ident(conn).and_then(|ident| {
                        sema.nameres_ident_in_scopes(
                            SyntaxTokenWithParent { parent: conn.syntax(), tok: ident },
                            NameContext::Value,
                            &chain,
                        )
                        .unique()
                    })
                })
                .flatten();
            if let DefinitionClass::Definition(port) = class {
                conn_port_by_name.insert(shape.name_range, port.clone());
            }
            ReferenceContext::ConnName {
                ident_range: shape.ident_range,
                collapse_range: shape.collapse_range,
                shorthand: false,
                side: ConnSide::Port,
                paired,
            }
        }
    }
}

/// True when the token sits at one of the syntax positions where
/// `DefinitionClass::resolve_in` diverges from plain value-identifier
/// resolution. Those positions are the direct token children of the listed
/// nodes (member access fields, module-like declaration names, instantiation
/// type names, package import names, named parameter/port connection names)
/// and identifiers wrapped in a `Name` node under a scoped name, a named
/// type (they select the Type name context) or a checker instantiation
/// (its type name resolves in the Type namespace).
///
/// Every check is O(1) on the token's parent (and grandparent); the subtree
/// walk from the old fast-path gate was dropped because it also flagged every
/// token inside a module body, which made the fast path dead on module-heavy
/// files.
fn token_in_special_context(
    SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent<'_>,
) -> bool {
    if ast::MemberAccessExpression::cast(parent).is_some_and(|node| node.name() == Some(tok))
        || ast::ModuleHeader::cast(parent).is_some_and(|node| node.name() == Some(tok))
        || ast::PrimitiveInstantiation::cast(parent).is_some_and(|node| node.type_() == Some(tok))
        || ast::HierarchyInstantiation::cast(parent).is_some_and(|node| node.type_() == Some(tok))
        || ast::PackageImportItem::cast(parent)
            .is_some_and(|node| node.package() == Some(tok) || node.item() == Some(tok))
        || ast::NamedParamAssignment::cast(parent).is_some_and(|node| node.name() == Some(tok))
        || ast::NamedPortConnection::cast(parent).is_some_and(|node| node.name() == Some(tok))
    {
        return true;
    }

    // Identifier tokens are wrapped in a `Name` node; the divergent context is
    // the Name's parent.
    if !ast::Name::can_cast(parent.kind()) {
        return false;
    }
    let Some(grandparent) = parent.parent() else {
        return false;
    };
    if ast::ScopedName::can_cast(grandparent.kind()) || ast::NamedType::can_cast(grandparent.kind())
    {
        return true;
    }
    ast::CheckerInstantiation::cast(grandparent)
        .is_some_and(|node| rightmost_name_token(node.type_()) == Some(tok))
}

fn collect_definition_token(
    db: &dyn WorkspaceSymbolIndexDb,
    definition: DefId,
    file_id: FileId,
    range: TextRange,
    token: SyntaxTokenWithParent<'_>,
    context: &ReferenceContext,
    groups: &mut FxHashMap<DefId, FileReferenceGroup>,
) {
    let origins = definition.origins(db);
    let Some(name) = origins.iter().find_map(|origin| origin.name(db)) else {
        return;
    };
    let definition_ranges = definition_ranges_for(db, definition.clone());
    let is_definition_site = definition_ranges.iter().any(|definition_range| {
        definition_range.file_id == file_id && definition_range.range == range
    });
    if is_definition_site {
        return;
    }

    let group = groups
        .entry(definition.clone())
        .or_insert_with(|| FileReferenceGroup { name: name.to_string(), references: Vec::new() });
    let reference = SemanticReference {
        file_id,
        range,
        category: ReferenceCategory::from_tok(token),
        ptr: SyntaxTokenPtr::from_token(token),
        context: context.clone(),
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
                .filter_map(|def_id| def_id.primary_origin(db).as_module())
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

impl FileModuleEdges {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let mut edges = Vec::new();
        for (_, defs) in db.file_scope(hir_file_id).iter_listing() {
            for def_id in defs.iter().filter(|def_id| def_id.kind(db).is_instantiable_def()) {
                let Some(caller) = def_id.primary_origin(db).as_module() else {
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

    let mut edges = Vec::new();
    for source_root_id in db.workspace_source_root_ids().iter().copied() {
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
    crate::token::name_precedence(kind)
}

#[cfg(test)]
mod tests {
    use hir_semantics::semantics::SemanticsImpl;
    use syntax::SyntaxElement;
    use utils::line_index::TextSize;

    use super::*;
    use crate::test_utils::setup_marked;

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
        let hir_file_id = HirFileId::from(file_id);
        let tree = db.parse(hir_file_id);
        let root = tree.root().expect("test source should parse");
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
                    let expected = sema
                        .container_for_node(hir_file_id, token.parent)
                        .unwrap_or(hir_file_id.into());
                    if cached != expected
                        && let (ArenaOwnerId::GenerateBlock(a), ArenaOwnerId::GenerateBlock(b)) =
                            (cached.clone(), expected.clone())
                    {
                        eprintln!("cached loc cont_id={:?} src={:?}", a.loc().cont_id, a.loc().src);
                        eprintln!(
                            "expected loc cont_id={:?} src={:?}",
                            b.loc().cont_id,
                            b.loc().src
                        );
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
        let hir_file_id = HirFileId::from(file_id);
        let tree = db.parse(hir_file_id);
        let root = tree.root().expect("test source should parse");
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
                    DefinitionClass::resolve_in(db, hir_file_id, token, Some(container.clone()))
                        .unique()
                } else {
                    let chain = chains.chain_for(db, container.clone());
                    sema.nameres_ident_in_scopes(token, NameContext::Value, &chain)
                        .map(DefinitionClass::Definition)
                        .unique()
                };
                let full =
                    DefinitionClass::resolve_in(db, hir_file_id, token, Some(container.clone()))
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
        let index = source_root_semantic_index_for_root(host.raw_db(), SourceRootId(0));

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
                .references_for_definition(paired.clone())
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
                .references_for_definition(paired.clone())
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
                .references_for_definition(paired.clone())
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
                .references_for_definition(paired.clone())
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
        let root = tree.root().expect("test source should parse");
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
        let index = source_root_semantic_index_for_root(host.raw_db(), SourceRootId(0));
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
