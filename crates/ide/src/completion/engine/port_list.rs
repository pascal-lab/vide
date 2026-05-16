use hir::{
    db::HirDb,
    hir_def::module::{ModuleId, ModuleSrc},
    scope::{ModuleEntry, UnitEntry},
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::{get::Get, text_edit::TextEditItem};

use super::{CompletionItem, CompletionItemKind};
use crate::completion::{
    context::{CompletionContext, PortListKind},
    port_keywords::port_item_keywords,
};

pub(super) fn complete_in_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    kind: PortListKind,
) -> Vec<CompletionItem> {
    match kind {
        PortListKind::Ansi => complete_ansi_port_list(db, position, prefix, ctx),
        PortListKind::Function => complete_function_port_list(db, position, prefix, ctx),
        PortListKind::NonAnsi => complete_non_ansi_port_list(db, position, prefix, ctx),
    }
}

fn complete_ansi_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let mut items = visible_typedefs_in_module_header(db, position)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect::<Vec<_>>();

    items.extend(
        port_item_keywords(PortListKind::Ansi).iter().filter(|kw| kw.starts_with(prefix)).map(
            |kw| CompletionItem {
                label: kw.clone(),
                kind: CompletionItemKind::Keyword,
                edit: Some(TextEditItem::replace(ctx.replacement, kw.clone())),
                snippet_edit: None,
            },
        ),
    );

    items
}

fn complete_function_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let mut items = visible_typedefs_in_module_header(db, position)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect::<Vec<_>>();

    items.extend(
        port_item_keywords(PortListKind::Function).iter().filter(|kw| kw.starts_with(prefix)).map(
            |kw| CompletionItem {
                label: kw.clone(),
                kind: CompletionItemKind::Keyword,
                edit: Some(TextEditItem::replace(ctx.replacement, kw.clone())),
                snippet_edit: None,
            },
        ),
    );

    items
}

fn visible_typedefs_in_module_header(db: &RootDb, position: FilePosition) -> Vec<String> {
    let sema = Semantics::new(db);
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let module = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, position.offset);
    let Some(module) = module else {
        return Vec::new();
    };
    let Some(file_id) = sema.find_file(module.syntax()) else {
        return Vec::new();
    };
    let (_, file_src_map) = db.hir_file_with_source_map(file_id);
    let module_src = ModuleSrc::from(module);
    let Some(module_id) = file_src_map.get(module_src).map(|id| ModuleId::new(file_id, id)) else {
        return Vec::new();
    };

    let mut names: Vec<String> = db
        .unit_scope()
        .iter()
        .filter_map(|(ident, entry)| matches!(entry, UnitEntry::FiledTypedefId(_)).then_some(ident))
        .map(|ident| ident.to_string())
        .collect();

    names.extend(
        db.module_scope(module_id)
            .iter()
            .filter_map(|(ident, entry)| {
                matches!(entry, ModuleEntry::TypedefId(_)).then_some(ident)
            })
            .map(|ident| ident.to_string()),
    );

    names.sort();
    names.dedup();
    names
}

fn complete_non_ansi_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let module = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, position.offset);
    let Some(module) = module else {
        return Vec::new();
    };
    let Some(file_id) = sema.find_file(module.syntax()) else {
        return Vec::new();
    };
    let (_, file_src_map) = db.hir_file_with_source_map(file_id);
    let module_src = ModuleSrc::from(module);
    let Some(module_id) = file_src_map.get(module_src).map(|id| ModuleId::new(file_id, id)) else {
        return Vec::new();
    };

    let scope = db.module_scope(module_id);
    scope
        .iter()
        .filter_map(|(ident, entry)| {
            matches!(
                entry,
                hir::scope::ModuleEntry::AnsiPortEntry(_)
                    | hir::scope::ModuleEntry::NonAnsiPortEntry(_)
            )
            .then_some(ident.to_string())
        })
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect()
}
