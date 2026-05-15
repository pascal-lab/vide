use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub(super) struct SnippetConfig {
    #[serde(default)]
    pub(super) top_level: BTreeMap<String, SnippetDef>,
    #[serde(default)]
    pub(super) module_header: BTreeMap<String, SnippetDef>,
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

pub(super) fn snippet_config() -> &'static SnippetConfig {
    static SNIPPETS: OnceLock<SnippetConfig> = OnceLock::new();
    SNIPPETS.get_or_init(|| {
        let manual_raw = include_str!("snippets.toml");
        parse_snippet_config(manual_raw).unwrap_or_else(|err| {
            tracing::error!(%err, "failed to parse bundled snippet config");
            SnippetConfig::default()
        })
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
    use super::*;

    #[test]
    fn bundled_snippets_parse() {
        let parsed = parse_snippet_config(include_str!("snippets.toml"));
        assert!(parsed.is_ok(), "snippets.toml failed to parse: {:?}", parsed.err());
    }
}
