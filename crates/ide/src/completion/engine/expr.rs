use std::collections::BTreeSet;

use hir::{
    container::InFile,
    db::{HirDb, InternDb},
    hir_def::{
        block::BlockId,
        module::{ModuleId, ModuleSrc},
        subroutine::{SubroutineId, SubroutineLoc, SubroutineSrc},
    },
    scope::{BlockEntry, ModuleEntry},
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
};
use utils::{
    get::Get,
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

    if let Some(block_id) = block_id_at_offset(&sema, root, position.offset) {
        collect_block_names(db, block_id, &mut names);
    }

    if let Some(subroutine_id) = subroutine_id_at_offset(db, &sema, root, position.offset) {
        collect_subroutine_names(db, subroutine_id, &mut names);
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
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<BlockId> {
    let block = sema.find_node_at_offset::<ast::BlockStatement>(root, offset)?;
    sema.block_to_def(block)
}

fn subroutine_id_at_offset(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<SubroutineId> {
    let func = sema.find_node_at_offset::<ast::FunctionDeclaration>(root, offset)?;
    let file_id = sema.find_file(func.syntax());
    let cont_id = module_id_at_offset(db, sema, root, offset).map_or(file_id.into(), Into::into);
    let src = SubroutineSrc::from(func);
    Some(db.intern_subroutine(SubroutineLoc { cont_id, src: InFile::new(file_id, src) }))
}

fn collect_block_names(db: &RootDb, block_id: BlockId, names: &mut BTreeSet<String>) {
    let scope = db.block_scope(block_id);
    for (ident, entry) in scope.iter() {
        if matches!(entry, BlockEntry::DeclId(_)) {
            names.insert(ident.to_string());
        }
    }
}

fn collect_subroutine_names(
    db: &RootDb,
    subroutine_id: SubroutineId,
    names: &mut BTreeSet<String>,
) {
    let subroutine = db.subroutine(subroutine_id);
    for port in subroutine.ports.iter() {
        if let Some(name) = port.name.as_ref() {
            names.insert(name.to_string());
        }
    }
    for (_decl_id, decl) in subroutine.decls.iter() {
        if let Some(name) = decl.name.as_ref() {
            names.insert(name.to_string());
        }
    }
}

fn collect_module_names(db: &RootDb, module_id: ModuleId, names: &mut BTreeSet<String>) {
    let scope = db.module_scope(module_id);
    for (ident, entry) in scope.iter() {
        match entry {
            ModuleEntry::DeclId(_)
            | ModuleEntry::AnsiPortEntry(_)
            | ModuleEntry::NonAnsiPortEntry(_)
            | ModuleEntry::SubroutineId(_) => {
                names.insert(ident.to_string());
            }
            _ => {}
        }
    }
}
