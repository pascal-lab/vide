use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub(super) struct SnippetConfig {
    #[serde(default)]
    pub(super) top_level: BTreeMap<String, SnippetDef>,
    #[serde(default)]
    pub(super) module_item: BTreeMap<String, SnippetDef>,
    #[serde(default)]
    pub(super) directives: BTreeMap<String, SnippetDef>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum SnippetDef {
    Simple(String),
    Detailed { snippet: String, plain: Option<String> },
}

#[derive(Debug, Clone)]
pub(super) struct SnippetEntry {
    pub label: String,
    pub plain: String,
    pub snippet: String,
}

// The bundled snippet file is static project data; parse failures should
// surface during startup.
#[allow(clippy::expect_used)]
pub(super) fn snippet_config() -> &'static SnippetConfig {
    static SNIPPETS: OnceLock<SnippetConfig> = OnceLock::new();
    SNIPPETS.get_or_init(|| {
        let manual_raw = include_str!("snippets.toml");
        parse_snippet_config(manual_raw).expect("bundled snippets.toml must parse")
    })
}

pub(super) fn entries(map: &BTreeMap<String, SnippetDef>) -> Vec<SnippetEntry> {
    map.iter().map(|(label, def)| def.to_entry(label)).collect()
}

fn parse_snippet_config(raw: &str) -> Result<SnippetConfig, toml::de::Error> {
    toml::from_str(raw)
}

impl SnippetDef {
    fn to_entry(&self, label: &str) -> SnippetEntry {
        match self {
            SnippetDef::Simple(snippet) => SnippetEntry {
                label: label.to_string(),
                plain: label.to_string(),
                snippet: snippet.clone(),
            },
            SnippetDef::Detailed { snippet, plain } => SnippetEntry {
                label: label.to_string(),
                plain: plain.clone().unwrap_or_else(|| label.to_string()),
                snippet: snippet.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use syntax::SyntaxToken;
    use utils::line_index::{TextRange, TextSize};

    use super::*;
    use crate::completion::{context::ExpectedSyntax, syntax_keywords};

    #[test]
    fn bundled_snippets_parse() {
        let parsed = parse_snippet_config(include_str!("snippets.toml"));
        assert!(parsed.is_ok(), "snippets.toml failed to parse: {:?}", parsed.err());
    }

    #[test]
    fn bundled_snippets_are_gated_by_token_prediction_contexts() {
        let snippets = parse_snippet_config(include_str!("snippets.toml")).unwrap();

        for entry in entries(&snippets.top_level) {
            assert!(
                predicted_in(ExpectedSyntax::CompilationUnitItem, "/*caret*/\n", &entry.plain),
                "top-level snippet `{}` uses plain `{}` which is not source-predicted",
                entry.label,
                entry.plain
            );
        }

        let source_contexts = [
            (ExpectedSyntax::ModuleItem, "module m;\n  /*caret*/\nendmodule\n"),
            (
                ExpectedSyntax::GenerateItem,
                "module m; generate\n  /*caret*/\nendgenerate endmodule\n",
            ),
            (ExpectedSyntax::SpecifyItem, "module m; specify\n  /*caret*/\nendspecify endmodule\n"),
            (
                ExpectedSyntax::BlockItem { declarations_allowed: true },
                "module m; initial begin\n  /*caret*/\nend endmodule\n",
            ),
            (ExpectedSyntax::Statement, "module m; initial begin\n  /*caret*/\nend endmodule\n"),
        ];

        for entry in entries(&snippets.module_item) {
            assert!(
                source_contexts
                    .iter()
                    .any(|(expected, text)| { predicted_in(*expected, text, &entry.plain) }),
                "module snippet `{}` uses plain `{}` which is not source-predicted; keyword kind: {:?}",
                entry.label,
                entry.plain,
                SyntaxToken::keyword_kind_for_version("1364-2005", &entry.plain)
            );
        }
    }

    fn predicted_in(expected: ExpectedSyntax, text: &str, plain: &str) -> bool {
        let caret = text.find("/*caret*/").unwrap();
        let source = text.replace("/*caret*/", "");
        let offset = TextSize::from(caret as u32);
        syntax_keywords::keyword_candidates(expected, &source, TextRange::empty(offset), plain)
            .contains_plain(plain)
    }
}
