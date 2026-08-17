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
    SyntaxAncestors, SyntaxNode, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use super::*;
use crate::{
    db::workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    definitions::{DefinitionClass, rightmost_name_token},
    module_resolution::resolve_hir_instantiation_target,
    references::search::resolve_source_range,
};

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
pub(crate) struct ContainerCache<'tree> {
    by_node: FxHashMap<SyntaxNode<'tree>, OwnerId>,
}

impl<'tree> ContainerCache<'tree> {
    pub(crate) fn new() -> Self {
        Self { by_node: FxHashMap::default() }
    }

    /// The container of a token: the nearest container node on its ancestor
    /// chain whose id computes successfully, mirroring
    /// `find_map(container_to_def)`; nodes that fail to lower are skipped.
    pub(crate) fn container_for(
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
            return Some(*id);
        }
        let id = container_id_for_node(sema, file_id, node, self)?;
        self.by_node.insert(node, id);
        Some(id)
    }
}

/// Resolved scope chains by container. The nameres fast path looks every
/// token up in its container's chain; resolving the chain once per container
/// avoids per-token salsa `scope_for` queries, whose memos revalidate against
/// every intervening query during the index build and recompute O(scope
/// size) on each miss.
pub(crate) struct ScopeChainCache {
    by_container: FxHashMap<OwnerId, Arc<ResolvedScopes>>,
}

impl ScopeChainCache {
    pub(crate) fn new() -> Self {
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
        let chain = Arc::new(ResolvedScopes::new(db, ScopeChain::from_inner(db, container)));
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
        || ast::AnonymousProgram::cast(*node).is_some()
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
        return sema.module_to_def(file_id, module);
    }
    let kind = if ast::CheckerDeclaration::cast(node).is_some() {
        Some(OwnerKind::Checker)
    } else if ast::AnonymousProgram::cast(node).is_some() {
        Some(OwnerKind::AnonymousProgram)
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
pub(crate) fn reference_context(
    db: &dyn WorkspaceSymbolIndexDb,
    sema: &SemanticsImpl<'_>,
    context: &crate::semantic_index::SemanticSnapshotInputs,
    file_id: HirFileId,
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
    match role {
        ConnTokenRole::Data(conn) => {
            let Some(shape) = conn_shape(conn) else {
                return ReferenceContext::Plain;
            };
            let paired = is_same_name_conn(text, &shape)
                .then(|| {
                    if let Some(port) = conn_port_by_name.get(&shape.name_range) {
                        return Some(*port);
                    }
                    let name = conn.name()?;
                    let name_token = SyntaxTokenWithParent { parent: conn.syntax(), tok: name };
                    match DefinitionClass::resolve_in(
                        db,
                        context,
                        file_id,
                        name_token,
                        Some(container),
                    )
                    .unique()?
                    {
                        DefinitionClass::Definition(port) => Some(port),
                        DefinitionClass::PortConnShorthand { port, .. } => Some(port),
                    }
                })
                .flatten();
            ReferenceContext::ConnData {
                name_range: shape.name_range,
                collapse_range: shape.collapse_range,
                paired,
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
                            ConnSide::Port => Some(*local),
                            ConnSide::Local => Some(*port),
                        };
                        (side, paired)
                    }
                    DefinitionClass::Definition(def) => {
                        // One-sided shorthand resolution: the local side is the
                        // definition when plain value resolution matches it.
                        let chain = chains.chain_for(db, container);
                        let is_local = sema
                            .nameres_ident_in_scopes(token, NameContext::Value, &chain, None)
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
                    let chain = chains.chain_for(db, container);
                    conn_data_ident(conn).and_then(|ident| {
                        sema.nameres_ident_in_scopes(
                            SyntaxTokenWithParent { parent: conn.syntax(), tok: ident },
                            NameContext::Value,
                            &chain,
                            None,
                        )
                        .unique()
                    })
                })
                .flatten();
            if let DefinitionClass::Definition(port) = class {
                conn_port_by_name.insert(shape.name_range, *port);
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
pub(crate) fn token_in_special_context(
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

pub(crate) fn definition_class_for_token(
    db: &dyn WorkspaceSymbolIndexDb,
    sema: &SemanticsImpl<'_>,
    context: &crate::semantic_index::SemanticSnapshotInputs,
    file_id: HirFileId,
    token: SyntaxTokenWithParent<'_>,
    container: OwnerId,
    special: bool,
    chains: &mut ScopeChainCache,
) -> Option<DefinitionClass> {
    if special {
        DefinitionClass::resolve_in(db, context, file_id, token, Some(container)).unique()
    } else {
        let chain = chains.chain_for(db, container);
        sema.nameres_ident_in_scopes_at(file_id, token, NameContext::Value, &chain)
            .map(DefinitionClass::Definition)
            .unique()
    }
}

/// Definition name ranges of `definition` mapped to user-facing files, in
/// origin order. File-level callers own memoization for this pure projection.
#[salsa::tracked(returns(clone))]
fn definition_ranges(
    db: &dyn WorkspaceSymbolIndexDb,
    key: crate::db::DefinitionRangeKey,
) -> Vec<SemanticDefinitionRange> {
    let definition = key.def_id(db);
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

pub(crate) fn definition_ranges_for(
    db: &dyn WorkspaceSymbolIndexDb,
    definition: DefId,
) -> Vec<SemanticDefinitionRange> {
    definition_ranges(db, crate::db::DefinitionRangeKey::new(db, definition))
}

impl FileModuleIndex {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let item_tree = db.item_tree(hir_file_id);
        let modules = item_tree
            .module_headers()
            .filter(|header| header.kind().is_instantiable())
            .filter_map(|header| SemanticModuleDefinition::from_header(db, hir_file_id, header))
            .collect();
        Self { modules }
    }
}

impl FileModuleEdges {
    pub(crate) fn for_file(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Self {
        let module_indexes: Vec<_> = db
            .workspace_source_root_ids()
            .into_iter()
            .map(|root| {
                (
                    root,
                    crate::db::workspace_symbol_index_db::source_root_module_index_for_root(
                        db, root,
                    ),
                )
            })
            .collect();
        Self::for_file_with_indexes(db, file_id, &module_indexes)
    }

    pub(crate) fn for_file_with_indexes(
        db: &dyn WorkspaceSymbolIndexDb,
        file_id: FileId,
        module_indexes: &[(SourceRootId, Arc<ModuleIndex>)],
    ) -> Self {
        let hir_file_id = HirFileId::from(file_id);
        let item_tree = db.item_tree(hir_file_id);
        let mut edges = Vec::new();
        for header in item_tree.module_headers().filter(|header| header.kind().is_instantiable()) {
            let caller = header.owner();
            let Some(caller_def) = SemanticModuleDefinition::from_header(db, hir_file_id, header)
            else {
                continue;
            };
            let module = db.body_with_source_map(caller);
            for (instantiation_id, instantiation) in module.instantiations.iter() {
                let Some(callee_module_id) =
                    resolve_hir_instantiation_target(db, module_indexes, file_id, instantiation)
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
        Self { edges }
    }
}
