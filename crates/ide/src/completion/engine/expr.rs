use std::collections::BTreeMap;

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
    SyntaxNode,
    ast::{self, AstNode},
};
use utils::{
    get::Get,
    text_edit::{TextEditItem, TextSize},
};

use super::{CompletionItem, CompletionItemKind};
use crate::completion::context::CompletionContext;

#[derive(Clone, Copy, Debug)]
enum NameKind {
    Value,
    SubroutineCall,
}

pub(super) fn complete_expression(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    complete_expression_impl(db, position, prefix, ctx)
}

pub(super) fn complete_argument_exprs(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    complete_expression_impl(db, position, prefix, ctx)
}

fn complete_expression_impl(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let root = sema.parse_root(position.file_id);

    let mut names: BTreeMap<String, NameKind> = BTreeMap::new();

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
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, kind)| match kind {
            NameKind::Value => CompletionItem {
                label: name.clone(),
                kind: CompletionItemKind::Text,
                edit: Some(TextEditItem::replace(ctx.replacement, name)),
                snippet_edit: None,
            },
            NameKind::SubroutineCall => CompletionItem {
                label: name.clone(),
                kind: CompletionItemKind::Snippet,
                edit: Some(TextEditItem::replace(ctx.replacement, format!("{name}()"))),
                snippet_edit: Some(TextEditItem::replace(
                    ctx.replacement,
                    format!("{name}(${{1:args}})"),
                )),
            },
        })
        .collect()
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

fn collect_block_names(db: &RootDb, block_id: BlockId, names: &mut BTreeMap<String, NameKind>) {
    let scope = db.block_scope(block_id);
    for (ident, entry) in scope.iter() {
        if matches!(entry, BlockEntry::DeclId(_)) {
            names.entry(ident.to_string()).or_insert(NameKind::Value);
        }
    }
}

fn collect_subroutine_names(
    db: &RootDb,
    subroutine_id: SubroutineId,
    names: &mut BTreeMap<String, NameKind>,
) {
    let subroutine = db.subroutine(subroutine_id);
    for port in subroutine.ports.iter() {
        if let Some(name) = port.name.as_ref() {
            names.entry(name.to_string()).or_insert(NameKind::Value);
        }
    }
    for (_decl_id, decl) in subroutine.decls.iter() {
        if let Some(name) = decl.name.as_ref() {
            names.entry(name.to_string()).or_insert(NameKind::Value);
        }
    }
}

fn collect_module_names(db: &RootDb, module_id: ModuleId, names: &mut BTreeMap<String, NameKind>) {
    let scope = db.module_scope(module_id);
    for (ident, entry) in scope.iter() {
        match entry {
            ModuleEntry::DeclId(_)
            | ModuleEntry::AnsiPortEntry(_)
            | ModuleEntry::NonAnsiPortEntry(_) => {
                names.entry(ident.to_string()).or_insert(NameKind::Value);
            }
            ModuleEntry::SubroutineId(_) => {
                names.entry(ident.to_string()).or_insert(NameKind::SubroutineCall);
            }
            _ => {}
        }
    }
}
