use hir::{
    UnitEntry,
    completion::{CompletionEntryKind, CompletionScope, ModuleScopeCompletionExt},
    db::HirDb,
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use syntax::ast::{self, AstNode};

use crate::completion::{
    CompletionConfig, CompletionContext, CompletionItem, CompletionItemKind, compute_score,
};

/// Complete port connections in module instantiations
pub(crate) fn complete_port_connection(
    db: &RootDb,
    ctx: &CompletionContext,
    config: &CompletionConfig,
) -> Vec<CompletionItem> {
    if ctx.is_dot_completion() {
        return complete_port_names(db, ctx, config);
    }

    complete_connection_values(db, ctx)
}

fn complete_port_names(
    db: &RootDb,
    ctx: &CompletionContext,
    config: &CompletionConfig,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let sema = Semantics::new(db);
    let parse = sema.parse(ctx.position.file_id);
    let root = parse.syntax();

    let instance = sema.find_node_at_offset::<ast::HierarchicalInstance>(root, ctx.position.offset);
    let instance = match instance {
        Some(inst) => inst,
        None => return items,
    };

    let parent = instance.syntax().parent();
    let instantiation = parent.and_then(ast::HierarchyInstantiation::cast);
    let instantiation = match instantiation {
        Some(inst) => inst,
        None => return items,
    };

    let type_name = instantiation.type_();
    let type_name = match type_name {
        Some(name) => name,
        None => return items,
    };

    let type_name_str = type_name.value_text().to_string();
    let module_name = SmolStr::new(type_name_str.clone());

    let mut connected_ports = FxHashSet::default();
    let connections = instance.connections();

    for conn in connections.children() {
        if let Some(named_conn) = ast::NamedPortConnection::cast(conn.syntax())
            && let Some(port_name) = named_conn.name()
        {
            connected_ports.insert(port_name.value_text().to_string());
        }
    }

    let unit_scope = sema.db.unit_scope();
    let Some(entry) = unit_scope.get(&module_name) else {
        return items;
    };

    let module_id = match entry {
        UnitEntry::ModuleId(module_id) => module_id,
        UnitEntry::FiledDeclId(_)
        | UnitEntry::TypedefId(_)
        | UnitEntry::ClassId(_)
        | UnitEntry::PackageId(_)
        | UnitEntry::SubroutineId(_) => {
            return items;
        }
    };

    let module_scope = sema.db.module_scope(module_id);
    let completions = module_scope.collect_completions(sema.db, module_id);

    for entry in completions {
        if entry.kind != CompletionEntryKind::Port {
            continue;
        }

        let name = entry.name.to_string();
        if connected_ports.contains(&name) {
            continue;
        }

        let use_snippet = config.enable_snippets;

        items.push(CompletionItem {
            score: compute_score(
                ctx.prefix(),
                &name,
                if use_snippet { CompletionItemKind::Snippet } else { CompletionItemKind::Field },
                Some(ctx),
                Some(CompletionScope::Module),
            ),
            label: name.clone(),
            label_detail: None,
            detail: entry.detail.clone(),
            insert_text: if use_snippet { Some(format!("{}($0)", name)) } else { None },
            filter_text: None,
            kind: if use_snippet { CompletionItemKind::Snippet } else { CompletionItemKind::Field },
            primary_edit: None,
            additional_edits: Vec::new(),
        });
    }

    items
}

fn complete_connection_values(db: &RootDb, ctx: &CompletionContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    let sema = Semantics::new(db);
    let scope_entries = sema.scope_completions(ctx.position.file_id, ctx.position.offset);

    for scoped in scope_entries {
        let entry = scoped.entry;
        let (kind, allow) = match entry.kind {
            CompletionEntryKind::Variable
            | CompletionEntryKind::Net
            | CompletionEntryKind::Port => (CompletionItemKind::Variable, true),
            CompletionEntryKind::Instance => (CompletionItemKind::Module, true),
            _ => (CompletionItemKind::Identifier, false),
        };

        if !allow {
            continue;
        }

        let label = entry.name.to_string();
        items.push(CompletionItem {
            score: compute_score(ctx.prefix(), &label, kind, Some(ctx), Some(scoped.scope)),
            label,
            label_detail: None,
            detail: entry.detail.clone(),
            insert_text: None,
            filter_text: None,
            kind,
            primary_edit: None,
            additional_edits: Vec::new(),
        });
    }

    items
}
