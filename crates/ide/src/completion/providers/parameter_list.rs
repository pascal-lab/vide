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
use utils::text_edit::{TextEdit, TextEditItem, TextRange, TextSize};

use crate::completion::{
    CompletionConfig, CompletionContext, CompletionItem, CompletionItemKind, compute_score,
};

/// Complete parameter assignments within module instantiations
pub(crate) fn complete_parameter_list(
    db: &RootDb,
    ctx: &CompletionContext,
    config: &CompletionConfig,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let parse = sema.parse(ctx.position.file_id);
    let root = parse.syntax();

    let instantiation =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(root, ctx.position.offset);

    let Some(instantiation) = instantiation else {
        if ctx.is_parameter_trigger() {
            let token = &ctx.token;
            let token_len = token.text.len();
            let start = ctx
                .position
                .offset
                .checked_sub(TextSize::from(token_len as u32))
                .unwrap_or(ctx.position.offset);
            let end = ctx.position.offset;
            let range = TextRange::new(start, end);

            let mut snippet = if config.enable_snippets {
                let label = "#(...)".to_string();
                let insert_text = "#($0)".to_string();

                CompletionItem {
                    score: 0,
                    label,
                    label_detail: None,
                    detail: Some("parameter list".to_string()),
                    insert_text: Some(insert_text.clone()),
                    filter_text: None,
                    kind: CompletionItemKind::Snippet,
                    primary_edit: Some(TextEdit::from_iter([TextEditItem {
                        del: range,
                        ins: insert_text,
                    }])),
                    additional_edits: Vec::new(),
                }
            } else {
                let label = "#()".to_string();
                let insert_text = label.clone();

                CompletionItem {
                    score: 0,
                    label,
                    label_detail: None,
                    detail: Some("parameter list".to_string()),
                    insert_text: Some(insert_text.clone()),
                    filter_text: None,
                    kind: CompletionItemKind::Keyword,
                    primary_edit: Some(TextEdit::from_iter([TextEditItem {
                        del: range,
                        ins: insert_text,
                    }])),
                    additional_edits: Vec::new(),
                }
            };

            snippet.score =
                compute_score(ctx.prefix(), &snippet.label, snippet.kind, Some(ctx), None);
            if snippet.kind == CompletionItemKind::Snippet {
                snippet.score += 50;
            }
            return vec![snippet];
        }

        return Vec::new();
    };

    let Some(type_name) = instantiation.type_() else {
        return Vec::new();
    };

    let module_name = SmolStr::new(type_name.value_text().to_string());

    let mut assigned = FxHashSet::default();
    if let Some(params) = instantiation.parameters() {
        for param in params.parameters().children() {
            if let ast::ParamAssignment::NamedParamAssignment(named) = param
                && let Some(name) = named.name()
            {
                assigned.insert(name.value_text().to_string());
            }
        }
    }

    let unit_scope = sema.db.unit_scope();
    let Some(entry) = unit_scope.get(&module_name) else {
        return Vec::new();
    };

    let module_id = match entry {
        UnitEntry::ModuleId(module_id) => module_id,
        UnitEntry::FiledDeclId(_)
        | UnitEntry::TypedefId(_)
        | UnitEntry::ClassId(_)
        | UnitEntry::PackageId(_)
        | UnitEntry::SubroutineId(_) => {
            return Vec::new();
        }
    };

    let normalized_prefix = ctx.prefix().trim_start_matches(['.', '#']);

    let module_scope = sema.db.module_scope(module_id);
    let completions = module_scope.collect_completions(sema.db, module_id);

    let use_snippet = config.enable_snippets;
    let has_dot = ctx.is_dot_completion();

    let mut items = Vec::new();

    for entry in completions {
        if entry.kind != CompletionEntryKind::Parameter {
            continue;
        }

        let name = entry.name.to_string();
        if assigned.contains(&name) {
            continue;
        }

        let detail = entry.detail.clone();

        let insert_text = if use_snippet {
            if has_dot { Some(format!("{}($0)", name)) } else { Some(format!(".{}($0)", name)) }
        } else if has_dot {
            None
        } else {
            Some(format!(".{}", name))
        };

        let kind =
            if use_snippet { CompletionItemKind::Snippet } else { CompletionItemKind::Variable };

        items.push(CompletionItem {
            score: compute_score(
                normalized_prefix,
                &name,
                kind,
                Some(ctx),
                Some(CompletionScope::Module),
            ),
            label: name,
            label_detail: None,
            detail,
            insert_text,
            filter_text: None,
            kind,
            primary_edit: None,
            additional_edits: Vec::new(),
        });
    }

    items
}
