use std::{collections::BTreeMap, sync::OnceLock};

use hir::db::HirDb;
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{SyntaxFacts, SyntaxKind, TokenKind};
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
    let mut items: Vec<_> = keywords_config()
        .all
        .iter()
        .filter(|kw| ctx.expectation.is_some_and(|exp| kw.is_allowed_at(exp.syntax)))
        .filter(|kw| kw.label.starts_with(prefix))
        .map(|kw| kw.to_completion(ctx.replacement))
        .collect();

    items.extend(module_instantiation_snippets(db, prefix, ctx));

    items
}

#[derive(Debug, Default)]
struct KeywordsConfig {
    all: Vec<Keyword>,
}

#[derive(Debug, Clone)]
struct Keyword {
    label: String,
    plain: String,
    snippet: Option<String>,
    kind: KeywordKind,
    roles: Vec<SyntaxRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyntaxRole {
    Expected(ExpectedSyntax),
    TopLevelItem(SyntaxKind),
    ModuleHeader,
    ModuleMember(SyntaxKind),
    BlockDeclaration,
    Statement(TokenKind),
}

#[derive(Debug, Clone, Copy)]
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
    fn is_allowed_at(&self, expected: ExpectedSyntax) -> bool {
        self.roles.iter().any(|role| role.is_allowed_at(expected))
    }

    fn to_completion(&self, replace: utils::text_edit::TextRange) -> CompletionItem {
        CompletionItem {
            label: self.label.clone(),
            kind: self.kind.to_completion_kind(),
            edit: Some(TextEditItem::replace(replace, self.plain.clone())),
            snippet_edit: self.snippet.as_ref().map(|s| TextEditItem::replace(replace, s.clone())),
        }
    }
}

impl SyntaxRole {
    fn is_allowed_at(self, expected: ExpectedSyntax) -> bool {
        match self {
            SyntaxRole::Expected(role_expected) => expected == role_expected,
            SyntaxRole::TopLevelItem(kind) => {
                expected == ExpectedSyntax::CompilationUnitItem
                    && SyntaxFacts::is_allowed_in_compilation_unit(kind)
            }
            SyntaxRole::ModuleHeader => expected == ExpectedSyntax::ModuleHeaderItem,
            SyntaxRole::ModuleMember(kind) => {
                expected == ExpectedSyntax::ModuleItem && SyntaxFacts::is_allowed_in_module(kind)
            }
            SyntaxRole::BlockDeclaration => match expected {
                ExpectedSyntax::BlockItem { declarations_allowed } => declarations_allowed,
                _ => false,
            },
            SyntaxRole::Statement(kind) => {
                matches!(expected, ExpectedSyntax::BlockItem { .. } | ExpectedSyntax::Statement)
                    && SyntaxFacts::is_possible_statement(kind)
            }
        }
    }
}

fn snippets_to_keywords(
    entries: Vec<snippets::SnippetEntry>,
    roles_for_label: fn(&str) -> Vec<SyntaxRole>,
) -> Vec<Keyword> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let roles = roles_for_label(&entry.label);
            (!roles.is_empty()).then_some(Keyword {
                label: entry.label,
                plain: entry.plain,
                snippet: Some(entry.snippet),
                kind: KeywordKind::Snippet,
                roles,
            })
        })
        .collect()
}

fn keyword(
    label: impl Into<String>,
    plain: impl Into<String>,
    snippet: Option<String>,
    kind: KeywordKind,
    roles: Vec<SyntaxRole>,
) -> Keyword {
    Keyword { label: label.into(), plain: plain.into(), snippet, kind, roles }
}

fn top_level_snippet_roles(label: &str) -> Vec<SyntaxRole> {
    match label {
        "module" | "macromodule" => vec![SyntaxRole::TopLevelItem(SyntaxKind::MODULE_DECLARATION)],
        "primitive" => vec![SyntaxRole::TopLevelItem(SyntaxKind::UDP_DECLARATION)],
        "config" => vec![SyntaxRole::TopLevelItem(SyntaxKind::CONFIG_DECLARATION)],
        "library" => vec![SyntaxRole::Expected(ExpectedSyntax::CompilationUnitItem)],
        _ => Vec::new(),
    }
}

fn module_item_snippet_roles(label: &str) -> Vec<SyntaxRole> {
    match label {
        "wire" | "tri" | "tri0" | "tri1" | "trireg" | "triand" | "trior" | "wand" | "wor"
        | "supply0" | "supply1" => vec![SyntaxRole::ModuleMember(SyntaxKind::NET_DECLARATION)],
        "reg" | "integer" | "real" | "realtime" | "time" | "event" => vec![
            SyntaxRole::ModuleMember(SyntaxKind::DATA_DECLARATION),
            SyntaxRole::BlockDeclaration,
        ],
        "parameter" | "localparam" => vec![
            SyntaxRole::ModuleMember(SyntaxKind::PARAMETER_DECLARATION_STATEMENT),
            SyntaxRole::BlockDeclaration,
        ],
        "specparam" => vec![SyntaxRole::ModuleMember(SyntaxKind::SPECPARAM_DECLARATION)],
        "defparam" => vec![SyntaxRole::ModuleMember(SyntaxKind::DEF_PARAM)],
        "genvar" => vec![SyntaxRole::ModuleMember(SyntaxKind::GENVAR_DECLARATION)],
        "generate" => vec![SyntaxRole::ModuleMember(SyntaxKind::GENERATE_REGION)],
        "function" => vec![SyntaxRole::ModuleMember(SyntaxKind::FUNCTION_DECLARATION)],
        "task" => vec![SyntaxRole::ModuleMember(SyntaxKind::TASK_DECLARATION)],
        "assign" => vec![
            SyntaxRole::ModuleMember(SyntaxKind::CONTINUOUS_ASSIGN),
            SyntaxRole::Statement(syntax::Token![assign]),
        ],
        "deassign" => vec![SyntaxRole::Statement(syntax::Token![deassign])],
        "force" => vec![SyntaxRole::Statement(syntax::Token![force])],
        "release" => vec![SyntaxRole::Statement(syntax::Token![release])],
        "always" | "always @(*)" => vec![SyntaxRole::ModuleMember(SyntaxKind::ALWAYS_BLOCK)],
        "initial" => vec![SyntaxRole::ModuleMember(SyntaxKind::INITIAL_BLOCK)],
        "begin" => vec![SyntaxRole::Statement(syntax::Token![begin])],
        "fork" => vec![SyntaxRole::Statement(syntax::Token![fork])],
        "if" | "ifelse" => vec![SyntaxRole::Statement(syntax::Token![if])],
        "case" => vec![SyntaxRole::Statement(syntax::Token![case])],
        "casez" => vec![SyntaxRole::Statement(syntax::Token![casez])],
        "casex" => vec![SyntaxRole::Statement(syntax::Token![casex])],
        "for" => vec![SyntaxRole::Statement(syntax::Token![for])],
        "while" => vec![SyntaxRole::Statement(syntax::Token![while])],
        "repeat" => vec![SyntaxRole::Statement(syntax::Token![repeat])],
        "forever" => vec![SyntaxRole::Statement(syntax::Token![forever])],
        "disable" => vec![SyntaxRole::Statement(syntax::Token![disable])],
        "wait" => vec![SyntaxRole::Statement(syntax::Token![wait])],
        "specify" => vec![SyntaxRole::ModuleMember(SyntaxKind::SPECIFY_BLOCK)],
        _ => Vec::new(),
    }
}

fn module_header_snippet_roles(_label: &str) -> Vec<SyntaxRole> {
    Vec::new()
}

fn generated_keyword_roles(label: &str) -> Option<Vec<SyntaxRole>> {
    let roles = match label {
        label if syntax_keywords::is_gate_type_keyword(label) => {
            vec![SyntaxRole::ModuleMember(SyntaxKind::PRIMITIVE_INSTANTIATION)]
        }
        "module" | "macromodule" => vec![SyntaxRole::TopLevelItem(SyntaxKind::MODULE_DECLARATION)],
        "primitive" => vec![SyntaxRole::TopLevelItem(SyntaxKind::UDP_DECLARATION)],
        "config" => vec![SyntaxRole::TopLevelItem(SyntaxKind::CONFIG_DECLARATION)],
        "library" | "liblist" => vec![SyntaxRole::Expected(ExpectedSyntax::CompilationUnitItem)],
        "input" | "output" | "inout" | "ref" | "signed" | "unsigned" => {
            vec![SyntaxRole::ModuleHeader]
        }
        "wire" | "tri" | "tri0" | "tri1" | "trireg" | "triand" | "trior" | "wand" | "wor"
        | "supply0" | "supply1" => {
            vec![SyntaxRole::ModuleHeader, SyntaxRole::ModuleMember(SyntaxKind::NET_DECLARATION)]
        }
        "reg" | "integer" | "real" | "realtime" | "time" | "event" => vec![
            SyntaxRole::ModuleHeader,
            SyntaxRole::ModuleMember(SyntaxKind::DATA_DECLARATION),
            SyntaxRole::BlockDeclaration,
        ],
        "parameter" | "localparam" => vec![
            SyntaxRole::ModuleHeader,
            SyntaxRole::ModuleMember(SyntaxKind::PARAMETER_DECLARATION_STATEMENT),
            SyntaxRole::BlockDeclaration,
        ],
        "specparam" => vec![SyntaxRole::ModuleMember(SyntaxKind::SPECPARAM_DECLARATION)],
        "defparam" => vec![SyntaxRole::ModuleMember(SyntaxKind::DEF_PARAM)],
        "genvar" => vec![SyntaxRole::ModuleMember(SyntaxKind::GENVAR_DECLARATION)],
        "generate" => vec![SyntaxRole::ModuleMember(SyntaxKind::GENERATE_REGION)],
        "function" => vec![SyntaxRole::ModuleMember(SyntaxKind::FUNCTION_DECLARATION)],
        "task" => vec![SyntaxRole::ModuleMember(SyntaxKind::TASK_DECLARATION)],
        "assign" => vec![
            SyntaxRole::ModuleMember(SyntaxKind::CONTINUOUS_ASSIGN),
            SyntaxRole::Statement(syntax::Token![assign]),
        ],
        "initial" => vec![SyntaxRole::ModuleMember(SyntaxKind::INITIAL_BLOCK)],
        "final" => vec![SyntaxRole::ModuleMember(SyntaxKind::FINAL_BLOCK)],
        "always" => vec![SyntaxRole::ModuleMember(SyntaxKind::ALWAYS_BLOCK)],
        "specify" => vec![SyntaxRole::ModuleMember(SyntaxKind::SPECIFY_BLOCK)],
        "deassign" => vec![SyntaxRole::Statement(syntax::Token![deassign])],
        "force" => vec![SyntaxRole::Statement(syntax::Token![force])],
        "release" => vec![SyntaxRole::Statement(syntax::Token![release])],
        "begin" => vec![SyntaxRole::Statement(syntax::Token![begin])],
        "fork" => vec![SyntaxRole::Statement(syntax::Token![fork])],
        "if" => vec![SyntaxRole::Statement(syntax::Token![if])],
        "case" => vec![SyntaxRole::Statement(syntax::Token![case])],
        "casez" => vec![SyntaxRole::Statement(syntax::Token![casez])],
        "casex" => vec![SyntaxRole::Statement(syntax::Token![casex])],
        "for" => vec![SyntaxRole::Statement(syntax::Token![for])],
        "while" => vec![SyntaxRole::Statement(syntax::Token![while])],
        "repeat" => vec![SyntaxRole::Statement(syntax::Token![repeat])],
        "forever" => vec![SyntaxRole::Statement(syntax::Token![forever])],
        "do" => vec![SyntaxRole::Statement(syntax::Token![do])],
        "foreach" => vec![SyntaxRole::Statement(syntax::Token![foreach])],
        "disable" => vec![SyntaxRole::Statement(syntax::Token![disable])],
        "wait" => vec![SyntaxRole::Statement(syntax::Token![wait])],
        "return" => vec![SyntaxRole::Statement(syntax::Token![return])],
        "break" => vec![SyntaxRole::Statement(syntax::Token![break])],
        "continue" => vec![SyntaxRole::Statement(syntax::Token![continue])],
        "assert" => vec![SyntaxRole::Statement(syntax::Token![assert])],
        "assume" => vec![SyntaxRole::Statement(syntax::Token![assume])],
        "cover" => vec![SyntaxRole::Statement(syntax::Token![cover])],
        _ => return None,
    };

    Some(roles)
}

fn generated_keywords() -> Vec<Keyword> {
    let mut keywords = syntax::SyntaxToken::keyword_table_for_version("1364-2005");
    keywords.sort();
    keywords.dedup();
    keywords
        .into_iter()
        .filter_map(|kw| {
            let roles = generated_keyword_roles(&kw)?;
            Some(keyword(kw.clone(), kw, None, KeywordKind::Keyword, roles))
        })
        .collect()
}

fn keywords_config() -> &'static KeywordsConfig {
    static KEYWORDS: OnceLock<KeywordsConfig> = OnceLock::new();
    KEYWORDS.get_or_init(|| {
        let manual = snippets::snippet_config();

        let snippets =
            snippets_to_keywords(snippets::entries(&manual.top_level), top_level_snippet_roles)
                .into_iter()
                .chain(snippets_to_keywords(
                    snippets::entries(&manual.module_header),
                    module_header_snippet_roles,
                ))
                .chain(snippets_to_keywords(
                    snippets::entries(&manual.module_item),
                    module_item_snippet_roles,
                ))
                .collect();

        KeywordsConfig { all: combine_keywords(generated_keywords(), snippets) }
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
