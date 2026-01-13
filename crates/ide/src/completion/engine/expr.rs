use std::collections::BTreeSet;

use hir::{
    container::ContainerId,
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockSrc},
        module::{ModuleId, ModuleSrc},
    },
    semantics::Semantics,
    scope::{BlockEntry, ModuleEntry},
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
};
use utils::{
    get::{Get, GetRef},
    text_edit::{TextEditItem, TextRange, TextSize},
};

use super::{CompletionItem, CompletionItemKind};
use crate::completion::context::CompletionContext;

pub(super) fn complete_expression(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    complete_expression_impl(db, position, prefix, ctx, true)
}

pub(super) fn complete_argument_exprs(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    complete_expression_impl(db, position, prefix, ctx, false)
}

fn complete_expression_impl(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    require_expr_node: bool,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let root = file.syntax();

    if require_expr_node && !is_in_expression(root, position.offset) {
        return Vec::new();
    }

    let mut names: BTreeSet<String> = BTreeSet::new();

    if let Some(block_id) = block_id_at_offset(db, &sema, root, position.offset) {
        collect_block_names(db, block_id, &mut names);
    }

    if let Some(module_id) = module_id_at_offset(db, &sema, root, position.offset) {
        collect_module_names(db, module_id, &mut names);
    }

    names
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect()
}

fn is_in_expression(root: SyntaxNode<'_>, offset: TextSize) -> bool {
    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return false;
    };

    SyntaxAncestors::start_from(node).any(|n| ast::Expression::can_cast(n.kind()))
}

fn module_id_at_offset(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<ModuleId> {
    let module = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, offset)?;
    let file_id = sema.find_file(module.syntax());
    let (_, file_src_map) = db.hir_file_with_source_map(file_id);
    let module_src = ModuleSrc::from(module);
    Some(ModuleId::new(file_id, file_src_map.get(module_src)))
}

fn block_id_at_offset(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<BlockId> {
    let block = sema.find_node_at_offset::<ast::BlockStatement>(root, offset)?;
    block_to_def(db, sema, block)
}

fn block_to_def(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    block: ast::BlockStatement<'_>,
) -> Option<BlockId> {
    let file_id = sema.find_file(block.syntax());
    let block_src = BlockSrc::from(block);
    let parent_container = container_id_for_node(db, sema, block.syntax());
    block_id_from_src(db, parent_container, block_src)
}

fn container_id_for_node(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    node: SyntaxNode<'_>,
) -> ContainerId {
    let file_id = sema.find_file(node);
    container_id_for_node_inner(db, sema, file_id, node)
}

fn container_id_for_node_inner(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: hir::file::HirFileId,
    node: SyntaxNode<'_>,
) -> ContainerId {
    for anc in SyntaxAncestors::start_from(node).skip(1) {
        if let Some(module) = ast::ModuleDeclaration::cast(anc.clone()) {
            let (_, file_src_map) = db.hir_file_with_source_map(file_id);
            let module_src = ModuleSrc::from(module);
            let local_module_id = file_src_map.get(module_src);
            return ModuleId::new(file_id, local_module_id).into();
        }

        if let Some(block) = ast::BlockStatement::cast(anc.clone()) {
            let block_src = BlockSrc::from(block);
            let parent_container = container_id_for_node_inner(db, sema, file_id, block.syntax());
            if let Some(block_id) = block_id_from_src(db, parent_container, block_src) {
                return block_id.into();
            }
        }

        if ast::CompilationUnit::can_cast(anc.kind()) {
            return file_id.into();
        }
    }

    file_id.into()
}

fn block_id_from_src(
    db: &RootDb,
    container_id: ContainerId,
    block_src: BlockSrc,
) -> Option<BlockId> {
    match container_id {
        ContainerId::HirFileId(file_id) => {
            let (file, file_src_map) = db.hir_file_with_source_map(file_id);
            let local_block_id = file_src_map.get(block_src);
            Some(file.get(local_block_id).block_id)
        }
        ContainerId::ModuleId(module_id) => {
            let (module, module_src_map) = db.module_with_source_map(module_id);
            let local_block_id = module_src_map.get(block_src);
            Some(module.get(local_block_id).block_id)
        }
        ContainerId::BlockId(block_id) => {
            let (block, block_src_map) = db.block_with_source_map(block_id);
            let local_block_id = block_src_map.get(block_src);
            Some(block.get(local_block_id).block_id)
        }
    }
}

fn collect_block_names(db: &RootDb, block_id: BlockId, names: &mut BTreeSet<String>) {
    let scope = db.block_scope(block_id);
    for (ident, entry) in scope.iter() {
        if matches!(entry, BlockEntry::DeclId(_)) {
            names.insert(ident.to_string());
        }
    }
}

fn collect_module_names(db: &RootDb, module_id: ModuleId, names: &mut BTreeSet<String>) {
    let scope = db.module_scope(module_id);
    for (ident, entry) in scope.iter() {
        match entry {
            ModuleEntry::DeclId(_)
            | ModuleEntry::AnsiPortEntry(_)
            | ModuleEntry::NonAnsiPortEntry(_) => {
                names.insert(ident.to_string());
            }
            _ => {}
        }
    }
}
