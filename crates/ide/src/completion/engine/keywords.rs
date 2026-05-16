use hir::db::HirDb;
use ide_db::root_db::RootDb;
use span::FilePosition;

use super::candidate::CompletionCandidate;
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
    expected: ExpectedSyntax,
) -> Vec<CompletionCandidate> {
    let candidates = syntax_keywords::keyword_candidates(expected, prefix);

    let mut items: Vec<_> = candidates
        .labels()
        .iter()
        .map(|label| CompletionCandidate::keyword(label.clone(), ctx.replacement))
        .collect();

    items.extend(snippet_completions(&candidates, prefix, ctx));
    items.extend(module_instantiation_snippets(db, prefix, ctx, expected));

    items
}

fn module_instantiation_snippets(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
    expected: ExpectedSyntax,
) -> Vec<CompletionCandidate> {
    use hir::scope::UnitEntry;

    if expected != ExpectedSyntax::ModuleItem {
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
                CompletionCandidate::semantic_snippet(name.clone(), replace, plain, snippet),
                CompletionCandidate::semantic_snippet(
                    format!("{name} #(...)"),
                    replace,
                    plain_with_params,
                    snippet_with_params,
                ),
            ]
        })
        .collect()
}

fn snippet_completions(
    candidates: &syntax_keywords::KeywordCandidates,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let snippets = snippets::snippet_config();
    snippets::entries(&snippets.top_level)
        .into_iter()
        .chain(snippets::entries(&snippets.module_item))
        .filter(|entry| entry.label.starts_with(prefix))
        .filter(|entry| candidates.contains_plain(&entry.plain))
        .map(|entry| {
            CompletionCandidate::snippet(entry.label, ctx.replacement, entry.plain, entry.snippet)
        })
        .collect()
}
