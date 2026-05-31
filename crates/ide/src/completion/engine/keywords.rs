use hir::db::HirDb;

use super::candidate::CompletionCandidate;
use crate::{
    FilePosition,
    completion::{
        context::CompletionContext,
        engine::snippets,
        request::{KeywordProvider, KeywordSnippetScope},
        syntax_keywords,
    },
    db::root_db::RootDb,
};

pub(super) fn complete_keywords(
    db: &RootDb,
    _position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    provider: KeywordProvider,
) -> Vec<CompletionCandidate> {
    let candidates = syntax_keywords::keyword_candidates_for_context(provider.context, prefix);

    let mut items: Vec<_> = candidates
        .labels()
        .iter()
        .map(|label| CompletionCandidate::keyword(label.clone(), ctx.replacement))
        .collect();

    items.extend(snippet_completions(&candidates, prefix, ctx, provider.snippets));
    items.extend(module_instantiation_snippets(db, prefix, ctx, provider.module_instantiations));

    items
}

fn module_instantiation_snippets(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
    enabled: bool,
) -> Vec<CompletionCandidate> {
    use hir::scope::UnitEntry;

    if !enabled || prefix.is_empty() {
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
            let snippet = format!("{name} ${{1:u0}}(${{2}});");

            let plain_with_params = format!("{name} #() u0();");
            let snippet_with_params = format!("{name} #(${{1}}) ${{2:u0}}(${{3}});");

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
    scope: KeywordSnippetScope,
) -> Vec<CompletionCandidate> {
    let snippets = snippets::snippet_config();
    let entries = match scope {
        KeywordSnippetScope::None => Vec::new(),
        KeywordSnippetScope::CompilationUnit => snippets::entries(&snippets.top_level),
        KeywordSnippetScope::LibraryMap => snippets::entries(&snippets.library_map),
        KeywordSnippetScope::DesignItem => snippets::entries(&snippets.module_item),
        KeywordSnippetScope::ParameterPortList => snippets::entries(&snippets.parameter_port_list),
    };

    entries
        .into_iter()
        .filter(|entry| entry.label.starts_with(prefix))
        .filter(|entry| candidates.contains_plain(&entry.plain))
        .map(|entry| {
            CompletionCandidate::snippet(entry.label, ctx.replacement, entry.plain, entry.snippet)
        })
        .collect()
}

pub(super) fn complete_else_clause(
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    snippets::entries(&snippets::snippet_config().else_clause)
        .into_iter()
        .filter(|entry| entry.label.starts_with(prefix))
        .map(|entry| {
            CompletionCandidate::snippet(entry.label, ctx.replacement, entry.plain, entry.snippet)
        })
        .collect()
}
