use hir::{db::HirDb, hir_def::module::ModuleId, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::{
    get::{Get, GetRef},
    text_edit::{TextEditItem, TextRange},
};

use crate::completion::context::{
    CompletionContext, DotKind, LexContext, Qualifier, TriggerChar, completion_context,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub edit: Option<TextEditItem>,
}

pub fn completions(
    db: &RootDb,
    position: FilePosition,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    let ctx = completion_context(db, position, trigger);
    completions_with_context(db, position, &ctx)
}

fn completions_with_context(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if ctx.lex != LexContext::Code {
        return Vec::new();
    }

    match ctx.qualifier {
        Some(Qualifier::AfterDot(after_dot)) => match after_dot.kind {
            DotKind::NamedPort => {
                complete_named_port_names(db, position, &ctx.prefix, ctx.replacement)
            }
            DotKind::NamedParam => {
                complete_named_param_names(db, position, &ctx.prefix, ctx.replacement)
            }
            DotKind::Member => Vec::new(),
        },
        Some(Qualifier::AfterHash(_)) => Vec::new(),
        Some(Qualifier::InParenList(_)) => Vec::new(),
        Some(Qualifier::AfterAt(_)) => Vec::new(),
        Some(Qualifier::AfterBacktick) => Vec::new(),
        None => Vec::new(),
    }
}

fn complete_named_port_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    ports_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn complete_named_param_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    replacement: TextRange,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    overridable_params_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(replacement, name)),
        })
        .collect()
}

fn ports_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let module = db.module(module_id);
    let mut names = Vec::new();

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { ports, .. } => {
            for (_, port) in ports.iter() {
                if let Some(label) = port.label.as_ref() {
                    names.push(label.to_string());
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

fn overridable_params_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);

    let mut names = Vec::new();

    for (_decl_id, decl) in module.decls.iter() {
        if decl.name.is_none() {
            continue;
        }
        let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
            decl.parent
        else {
            continue;
        };
        let hir::hir_def::declaration::Declaration::ParamDecl(_) = module.get(declaration_id)
        else {
            continue;
        };

        let src = module_src_map.get(declaration_id);
        let hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr) = src else {
            continue;
        };
        let Some(ast_decl) = ptr.to_node(&tree).and_then(ast::ParameterDeclaration::cast) else {
            continue;
        };

        let Some(keyword) = ast_decl.keyword() else {
            continue;
        };
        if keyword.kind() != syntax::Token![parameter] {
            continue;
        }

        names.push(decl.name.as_ref().unwrap().to_string());
    }

    names.sort();
    names.dedup();
    names
}
