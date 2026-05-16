use hir::db::HirDb;
use ide_db::root_db::RootDb;
use span::FilePosition;
use utils::text_edit::TextEditItem;

use super::named::{CompletionItem, CompletionItemKind};
use crate::completion::{
    context::{CompletionContext, ExpectedSyntax},
    engine::snippets,
    syntax_keywords,
};

pub(super) fn complete_keywords(
    db: &RootDb,
    _position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(expectation) = ctx.expectation.map(|expectation| expectation.syntax) else {
        return Vec::new();
    };

    let mut items: Vec<_> = syntax_keywords::keywords_for_expected(expectation)
        .iter()
        .filter(|label| label.starts_with(prefix))
        .map(|label| CompletionItem {
            label: label.clone(),
            kind: CompletionItemKind::Keyword,
            edit: Some(TextEditItem::replace(ctx.replacement, label.clone())),
            snippet_edit: None,
        })
        .collect();

    items.extend(snippet_completions(expectation, prefix, ctx));
    items.extend(module_instantiation_snippets(db, prefix, ctx));

    items
}

fn module_instantiation_snippets(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    use hir::scope::UnitEntry;

    if !ctx.expectation.is_some_and(|expectation| expectation.syntax == ExpectedSyntax::ModuleItem)
    {
        return Vec::new();
    }

    if prefix.is_empty() {
        return Vec::new();
    }

    let mut modules: Vec<String> = db
        .unit_scope()
        .iter()
        .filter_map(|(ident, entry)| matches!(entry, UnitEntry::ModuleId(_)).then_some(ident))
        .map(|ident| ident.to_string())
        .filter(|name| name.starts_with(prefix))
        .collect();

    modules.sort();
    modules.dedup();

    let replace = ctx.replacement;
    modules
        .into_iter()
        .flat_map(|name| {
            let plain = format!("{name} u0();");
            let snippet = format!("{name} ${{1:u0}}(${{2:ports}});");

            let plain_with_params = format!("{name} #() u0();");
            let snippet_with_params = format!("{name} #(${{1:params}}) ${{2:u0}}(${{3:ports}});");

            [
                CompletionItem {
                    label: name.clone(),
                    kind: CompletionItemKind::Snippet,
                    edit: Some(TextEditItem::replace(replace, plain)),
                    snippet_edit: Some(TextEditItem::replace(replace, snippet)),
                },
                CompletionItem {
                    label: format!("{name} #(...)"),
                    kind: CompletionItemKind::Snippet,
                    edit: Some(TextEditItem::replace(replace, plain_with_params)),
                    snippet_edit: Some(TextEditItem::replace(replace, snippet_with_params)),
                },
            ]
        })
        .collect()
}

fn snippet_completions(
    expected: ExpectedSyntax,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let allowed = syntax_keywords::keywords_for_expected(expected);
    let snippets = snippets::snippet_config();
    snippets::entries(&snippets.top_level)
        .into_iter()
        .chain(snippets::entries(&snippets.module_item))
        .filter(|entry| entry.label.starts_with(prefix))
        .filter(|entry| allowed.iter().any(|keyword| keyword == &entry.plain))
        .map(|entry| CompletionItem {
            label: entry.label,
            kind: CompletionItemKind::Snippet,
            edit: Some(TextEditItem::replace(ctx.replacement, entry.plain)),
            snippet_edit: Some(TextEditItem::replace(ctx.replacement, entry.snippet)),
        })
        .collect()
}
