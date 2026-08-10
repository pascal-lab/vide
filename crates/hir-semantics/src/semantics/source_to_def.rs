use hir_def::{
    ast_id_map::SourceAstId,
    container::InFile,
    db::HirDefDb,
    owner::{OwnerId, OwnerKind},
};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNode,
    ast::{self, AstNode},
};

fn source_ast_id(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode<'_>,
) -> Option<SourceAstId> {
    let tree = db.parse(file_id);
    db.ast_id_map(file_id).id_of_node_in_tree(&tree, node)
}

fn owner_for_node(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode<'_>,
    kind: OwnerKind,
) -> Option<OwnerId> {
    let ast_id = source_ast_id(db, file_id, node)?;
    db.owner_table(file_id).owner_by_ast(ast_id, kind)
}

pub(super) fn module_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    module: ast::ModuleDeclaration<'_>,
) -> Option<OwnerId> {
    owner_for_node(db, file_id, module.syntax(), OwnerKind::Module)
}

pub(super) fn block_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    block: ast::BlockStatement<'_>,
) -> Option<OwnerId> {
    owner_for_node(db, file_id, block.syntax(), OwnerKind::Block)
}

pub(super) fn subroutine_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    subroutine: ast::FunctionDeclaration<'_>,
) -> Option<OwnerId> {
    owner_for_node(db, file_id, subroutine.syntax(), OwnerKind::Subroutine)
}

fn generate_owner_node(node: SyntaxNode<'_>) -> SyntaxNode<'_> {
    if ast::GenerateBlock::can_cast(node.kind())
        && let Some(parent) = node.parent()
        && ast::LoopGenerate::can_cast(parent.kind())
    {
        return parent;
    }
    node
}

fn owner_container(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode<'_>,
    kind: OwnerKind,
) -> Option<OwnerId> {
    owner_for_node(db, file_id, node, kind)
}

fn container_to_def(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    node: SyntaxNode<'_>,
) -> Option<OwnerId> {
    if ast::CompilationUnit::can_cast(node.kind()) {
        return db.owner_table(file_id).file_owner();
    }
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return owner_for_node(db, file_id, module.syntax(), OwnerKind::Module);
    }
    if ast::AnonymousProgram::cast(node).is_some() {
        return owner_for_node(db, file_id, node, OwnerKind::AnonymousProgram);
    }
    if ast::CheckerDeclaration::cast(node).is_some() {
        return owner_for_node(db, file_id, node, OwnerKind::Checker);
    }
    if ast::CovergroupDeclaration::cast(node).is_some() {
        return owner_for_node(db, file_id, node, OwnerKind::Covergroup);
    }
    if ast::ClockingDeclaration::cast(node).is_some() {
        return owner_for_node(db, file_id, node, OwnerKind::ClockingBlock);
    }
    if let Some(block) = ast::BlockStatement::cast(node) {
        return block_to_def(db, file_id, block);
    }
    if ast::ProceduralBlock::can_cast(node.kind()) {
        return owner_container(db, file_id, node, OwnerKind::ProceduralBlock);
    }
    if let Some(subroutine) = ast::FunctionDeclaration::cast(node) {
        return owner_for_node(db, file_id, subroutine.syntax(), OwnerKind::Subroutine);
    }
    if ast::GenerateBlock::can_cast(node.kind()) || ast::LoopGenerate::can_cast(node.kind()) {
        return owner_container(db, file_id, generate_owner_node(node), OwnerKind::GenerateBlock);
    }
    if ast::Member::can_cast(node.kind()) && is_generate_branch_member(node) {
        return owner_container(db, file_id, node, OwnerKind::GenerateBlock);
    }
    None
}

/// Whether a member is a single-member generate branch: it sits inside an
/// if/case generate and no stronger container (module, block, generate
/// region) separates it. Shared with the IDE semantic index build, which
/// mirrors the container dispatch and must not re-implement this predicate.
pub fn is_generate_branch_member(member: SyntaxNode<'_>) -> bool {
    for ancestor in SyntaxAncestors::start_from(member).skip(1) {
        if ast::IfGenerate::can_cast(ancestor.kind())
            || ast::CaseGenerate::can_cast(ancestor.kind())
        {
            return true;
        }

        if ast::GenerateBlock::can_cast(ancestor.kind())
            || ast::GenerateRegion::can_cast(ancestor.kind())
            || ast::ModuleDeclaration::can_cast(ancestor.kind())
            || ast::BlockStatement::can_cast(ancestor.kind())
            || ast::CheckerDeclaration::can_cast(ancestor.kind())
            || ast::CovergroupDeclaration::can_cast(ancestor.kind())
            || ast::ClockingDeclaration::can_cast(ancestor.kind())
        {
            return false;
        }
    }

    false
}

pub(super) fn find_container(
    db: &dyn HirDefDb,
    InFile { value: node, file_id }: InFile<SyntaxNode>,
) -> OwnerId {
    SyntaxAncestors::start_from(node)
        .skip(1) // skip the node itself
        .find_map(|node| container_to_def(db, file_id, node))
        .or_else(|| db.owner_table(file_id).file_owner())
        .expect("every syntax file must have a canonical owner")
}
