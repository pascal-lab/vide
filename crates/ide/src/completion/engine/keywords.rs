use std::{collections::BTreeMap, sync::OnceLock};

use hir::db::HirDb;
use ide_db::root_db::RootDb;
use serde::Deserialize;
use span::FilePosition;
use utils::text_edit::TextEditItem;

use super::named::{CompletionItem, CompletionItemKind};
use crate::completion::{
    context::{CompletionContext, SynContext},
    engine::snippets,
};

pub(super) fn complete_keywords(
    db: &RootDb,
    _position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if !matches!(ctx.syn, SynContext::TopLevel | SynContext::ModuleHeader | SynContext::ModuleItem)
    {
        return Vec::new();
    }

    let all = match ctx.syn {
        SynContext::TopLevel => top_level_keywords(),
        SynContext::ModuleHeader => module_header_keywords(),
        SynContext::ModuleItem => module_item_keywords(),
        _ => &[],
    };
    let all = if ctx.syn == SynContext::ModuleHeader && all.is_empty() {
        module_item_keywords()
    } else {
        all
    };

    let mut items: Vec<_> = all
        .iter()
        .filter(|kw| kw.label.starts_with(prefix))
        .map(|kw| kw.to_completion(ctx.replacement))
        .collect();

    items.extend(module_instantiation_snippets(db, prefix, ctx));

    items
}

#[derive(Debug, Deserialize, Default)]
struct KeywordsConfig {
    #[serde(default)]
    top_level: Vec<Keyword>,
    #[serde(default)]
    module_header: Vec<Keyword>,
    #[serde(default)]
    module_item: Vec<Keyword>,
}

#[derive(Debug, Deserialize)]
struct Keyword {
    label: String,
    plain: String,
    snippet: Option<String>,
    kind: KeywordKind,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KeywordKind {
    Keyword,
    Snippet,
}

impl KeywordKind {
    fn to_completion_kind(self) -> CompletionItemKind {
        match self {
            KeywordKind::Keyword => CompletionItemKind::Keyword,
            KeywordKind::Snippet => CompletionItemKind::Snippet,
        }
    }
}

impl Keyword {
    fn to_completion(&self, replace: utils::text_edit::TextRange) -> CompletionItem {
        CompletionItem {
            label: self.label.clone(),
            kind: self.kind.to_completion_kind(),
            edit: Some(TextEditItem::replace(replace, self.plain.clone())),
            snippet_edit: self.snippet.as_ref().map(|s| TextEditItem::replace(replace, s.clone())),
        }
    }
}
fn snippets_to_keywords(entries: Vec<snippets::SnippetEntry>) -> Vec<Keyword> {
    entries
        .into_iter()
        .map(|entry| Keyword {
            label: entry.label,
            plain: entry.plain,
            snippet: Some(entry.snippet),
            kind: KeywordKind::Snippet,
        })
        .collect()
}

fn top_level_keywords() -> &'static [Keyword] {
    &keywords_config().top_level
}

fn module_header_keywords() -> &'static [Keyword] {
    &keywords_config().module_header
}

fn module_item_keywords() -> &'static [Keyword] {
    &keywords_config().module_item
}

fn keywords_config() -> &'static KeywordsConfig {
    static KEYWORDS: OnceLock<KeywordsConfig> = OnceLock::new();
    KEYWORDS.get_or_init(|| {
        let manual = snippets::snippet_config();
        let generated = generated_keywords();

        KeywordsConfig {
            top_level: combine_keywords(
                Vec::new(),
                snippets_to_keywords(snippets::entries(&manual.top_level)),
            ),
            module_header: combine_keywords(
                Vec::new(),
                snippets_to_keywords(snippets::entries(&manual.module_header)),
            ),
            module_item: combine_keywords(
                generated,
                snippets_to_keywords(snippets::entries(&manual.module_item)),
            ),
        }
    })
}

fn combine_keywords(generated: Vec<Keyword>, snippets: Vec<Keyword>) -> Vec<Keyword> {
    let mut by_label: BTreeMap<String, Vec<Keyword>> = BTreeMap::new();
    for kw in generated {
        by_label.entry(kw.label.clone()).or_default().push(kw);
    }
    for kw in snippets {
        by_label.entry(kw.label.clone()).or_default().push(kw);
    }

    let mut combined = Vec::new();
    for (_label, mut entries) in by_label {
        entries.sort_by_key(keyword_sort_key);
        combined.extend(entries);
    }

    combined
}

fn generated_keywords() -> Vec<Keyword> {
    let mut keywords = syntax::keyword_table_for_version("1364-2005");
    if keywords.is_empty() {
        keywords = syntax::verilog_2005_keywords();
    }
    keywords.sort();
    keywords.dedup();
    keywords
        .into_iter()
        .map(|kw| Keyword {
            label: kw.clone(),
            plain: kw,
            snippet: None,
            kind: KeywordKind::Keyword,
        })
        .collect()
}

fn keyword_sort_key(keyword: &Keyword) -> u8 {
    match keyword.kind {
        KeywordKind::Keyword => 0,
        KeywordKind::Snippet => 1,
    }
}

fn module_instantiation_snippets(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    use hir::scope::UnitEntry;

    if ctx.syn != SynContext::ModuleItem {
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
