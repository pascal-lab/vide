use hir_def::{
    container::ScopeChain,
    def_id::DefId,
    owner::{OwnerId, OwnerKind},
    pathres::ResolvedScopes,
    symbol::NameContext,
};
use hir_semantics::semantics::SemanticsImpl;
use itertools::Itertools;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxAncestors, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTokenWithParent, WalkEvent,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
    ptr::SyntaxTokenPtr,
    token::TokenKindExt,
};
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use super::*;
use crate::{
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    definitions::{DefinitionClass, rightmost_name_token},
    module_resolution::resolve_hir_instantiation_target,
    references::{ReferenceCategory, search::resolve_source_range},
    semantic_target::{
        SemanticTarget, TargetIntent, preproc::emit_token_index,
        resolve_semantic_target_with_emitted,
    },
};

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
pub(super) struct ContainerCache<'tree> {
    by_node: FxHashMap<SyntaxNode<'tree>, OwnerId>,
}

impl<'tree> ContainerCache<'tree> {
    pub(super) fn new() -> Self {
        Self { by_node: FxHashMap::default() }
    }

    /// The container of a token: the nearest container node on its ancestor
    /// chain whose id computes successfully, mirroring
    /// `find_map(container_to_def)`; nodes that fail to lower are skipped.
    pub(super) fn container_for(
        &mut self,
        sema: &SemanticsImpl<'_>,
        file_id: HirFileId,
        token_parent: SyntaxNode<'tree>,
    ) -> OwnerId {
        for node in SyntaxAncestors::start_from(token_parent) {
            if is_container_node(&node)
                && let Some(id) = self.try_id_for(sema, file_id, node)
            {
                return id;
            }
        }
        sema.db.owner_table(file_id).file_owner().expect("file owner")
    }

    pub(super) fn try_id_for(
        &mut self,
        sema: &SemanticsImpl<'_>,
        file_id: HirFileId,
        node: SyntaxNode<'tree>,
    ) -> Option<OwnerId> {
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
pub(super) struct ScopeChainCache {
    by_container: FxHashMap<OwnerId, Arc<ResolvedScopes>>,
}

impl ScopeChainCache {
    pub(super) fn new() -> Self {
        Self { by_container: FxHashMap::default() }
    }

    pub(super) fn chain_for(
        &mut self,
        db: &dyn WorkspaceSymbolIndexDb,
        container: OwnerId,
    ) -> Arc<ResolvedScopes> {
        if let Some(chain) = self.by_container.get(&container) {
            return chain.clone();
        }
        let chain =
            Arc::new(ResolvedScopes::new(db, ScopeChain::from_inner(db, container.clone().into())));
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
        || ast::CheckerDeclaration::cast(*node).is_some()
        || ast::CovergroupDeclaration::cast(*node).is_some()
        || ast::ClockingDeclaration::cast(*node).is_some()
        || ast::BlockStatement::cast(*node).is_some()
        || ast::ProceduralBlock::cast(*node).is_some()
        || ast::FunctionDeclaration::cast(*node).is_some()
        || ast::CompilationUnit::cast(*node).is_some()
        || ast::GenerateBlock::cast(*node).is_some()
        || (ast::Member::cast(*node).is_some() && is_generate_branch_member(*node))
}

fn container_id_for_node<'tree>(
    sema: &SemanticsImpl<'_>,
    file_id: HirFileId,
    node: SyntaxNode<'tree>,
    _cache: &mut ContainerCache<'tree>,
) -> Option<OwnerId> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return Some(sema.module_to_def(file_id, module)?);
    }
    let kind = if ast::CheckerDeclaration::cast(node).is_some() {
        Some(OwnerKind::Checker)
    } else if ast::CovergroupDeclaration::cast(node).is_some() {
        Some(OwnerKind::Covergroup)
    } else if ast::ClockingDeclaration::cast(node).is_some() {
        Some(OwnerKind::ClockingBlock)
    } else if ast::ProceduralBlock::cast(node).is_some() {
        Some(OwnerKind::ProceduralBlock)
    } else if let Some(block) = ast::BlockStatement::cast(node) {
        return sema.block_to_def(file_id, block);
    } else if let Some(func) = ast::FunctionDeclaration::cast(node) {
        return sema.subroutine_to_def(file_id, func);
    } else if ast::CompilationUnit::cast(node).is_some() {
        return sema.db.owner_table(file_id).file_owner();
    } else if ast::GenerateBlock::cast(node).is_some()
        || (ast::Member::cast(node).is_some() && is_generate_branch_member(node))
    {
        Some(OwnerKind::GenerateBlock)
    } else {
        None
    }?;

    let owner_node = if kind == OwnerKind::GenerateBlock
        && ast::GenerateBlock::cast(node).is_some()
        && node.parent().is_some_and(|parent| ast::LoopGenerate::cast(parent).is_some())
    {
        node.parent()?
    } else {
        node
    };
    let tree = sema.db.parse(file_id);
    let ast_id = sema.db.ast_id_map(file_id).id_of_node_in_tree(&tree, owner_node)?;
    sema.db.owner_table(file_id).owner_by_ast(ast_id, kind)
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
    container: OwnerId,
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
    container: OwnerId,
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
pub(super) fn token_in_special_context(
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
pub(super) fn definition_ranges_for(
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
        for (_, defs) in db
            .scope_for(db.owner_table(hir_file_id).file_owner().expect("file owner"))
            .iter_listing()
        {
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

impl FileModuleEdges {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let mut edges = Vec::new();
        for (_, defs) in db
            .scope_for(db.owner_table(hir_file_id).file_owner().expect("file owner"))
            .iter_listing()
        {
            for def_id in defs.iter().filter(|def_id| def_id.kind(db).is_instantiable_def()) {
                let Some(caller) = def_id.primary_origin(db).as_module(db) else {
                    continue;
                };
                let Some(caller_def) = SemanticModuleDefinition::new(db, caller) else {
                    continue;
                };
                let module = db.body_with_source_map(caller);
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
                        .source_range(db, instantiation_id)
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
