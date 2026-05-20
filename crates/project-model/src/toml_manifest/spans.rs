use std::ops::Range;

use toml_edit::{ImDocument, Value};

#[derive(Debug, PartialEq, Eq)]
pub struct TomlManifestField {
    pub key: String,
    pub key_range: Range<usize>,
    pub value_range: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TomlManifestPath {
    pub key: String,
    pub value: String,
    pub value_range: Range<usize>,
    pub content_range: Range<usize>,
}

pub fn toml_manifest_fields(text: &str) -> Vec<TomlManifestField> {
    manifest_top_level_values(text)
        .into_iter()
        .filter_map(|(key, key_range, value)| {
            Some(TomlManifestField { key, key_range, value_range: value.span()? })
        })
        .collect()
}

pub fn toml_manifest_field_at_offset(text: &str, offset: usize) -> Option<TomlManifestField> {
    toml_manifest_fields(text)
        .into_iter()
        .find(|field| range_contains_offset(&field.key_range, offset))
}

pub fn toml_manifest_paths(text: &str) -> Vec<TomlManifestPath> {
    manifest_top_level_values(text)
        .into_iter()
        .filter(|(key, _, _)| MANIFEST_PATH_FIELDS.contains(&key.as_str()))
        .flat_map(|(key, _, value)| {
            let mut paths = Vec::new();
            if let Some(path) = manifest_string_value(&key, &value, text) {
                paths.push(path);
            }
            if let Some(array) = value.as_array() {
                paths.extend(
                    array.iter().filter_map(|value| manifest_string_value(&key, value, text)),
                );
            }
            paths
        })
        .collect()
}

pub fn toml_manifest_path_at_offset(text: &str, offset: usize) -> Option<TomlManifestPath> {
    toml_manifest_paths(text)
        .into_iter()
        .find(|path| range_contains_offset(&path.content_range, offset))
}

fn manifest_top_level_values(text: &str) -> Vec<(String, Range<usize>, Value)> {
    let Ok(document) = text.parse::<ImDocument<String>>() else {
        return Vec::new();
    };

    document
        .as_table()
        .get_values()
        .into_iter()
        .filter_map(|(keys, value)| {
            let [key] = keys.as_slice() else {
                return None;
            };
            Some((key.get().to_string(), key.span()?, value.clone()))
        })
        .collect()
}

fn manifest_string_value(key: &str, value: &Value, text: &str) -> Option<TomlManifestPath> {
    let value_range = value.span()?;
    let content_range = string_content_range(text, value_range.clone())?;

    Some(TomlManifestPath {
        key: key.to_string(),
        value: value.as_str()?.to_string(),
        value_range,
        content_range,
    })
}

fn string_content_range(text: &str, value_range: Range<usize>) -> Option<Range<usize>> {
    let raw = text.get(value_range.clone())?;
    let quote_len = if raw.starts_with("\"\"\"") || raw.starts_with("'''") {
        3
    } else if raw.starts_with('"') || raw.starts_with('\'') {
        1
    } else {
        return None;
    };
    (raw.len() >= quote_len * 2)
        .then_some(value_range.start + quote_len..value_range.end - quote_len)
}

fn range_contains_offset(range: &Range<usize>, offset: usize) -> bool {
    range.start <= offset && offset <= range.end
}

const MANIFEST_PATH_FIELDS: &[&str] = &["sources", "include_dirs", "libraries", "exclude"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_lookup_uses_toml_key_spans() {
        let toml = "sources = [\"rtl\"]\n";
        let field = toml_manifest_field_at_offset(toml, 1).unwrap();

        assert_eq!(field.key, "sources");
        assert_eq!(field.key_range, 0..7);
        assert_eq!(&toml[field.value_range], "[\"rtl\"]");
        assert!(toml_manifest_field_at_offset(toml, 12).is_none());
    }

    #[test]
    fn path_lookup_uses_toml_value_spans() {
        let toml = "sources = [\n  \"rtl/top.sv\",\n]\n";
        let offset = toml.find("top").unwrap();
        let path = toml_manifest_path_at_offset(toml, offset).unwrap();

        assert_eq!(path.key, "sources");
        assert_eq!(path.value, "rtl/top.sv");
        assert_eq!(&toml[path.content_range.clone()], "rtl/top.sv");
        assert!(toml_manifest_path_at_offset(toml, 1).is_none());
    }

    #[test]
    fn paths_list_top_level_path_arrays() {
        let toml = "sources = [\"rtl\", \"ip\"]\ntop_modules = [\"top\"]\n";
        let paths = toml_manifest_paths(toml);
        let values = paths.iter().map(|path| path.value.as_str()).collect::<Vec<_>>();

        assert_eq!(values, ["rtl", "ip"]);
    }

    #[test]
    fn path_lookup_ignores_non_path_fields() {
        let toml = "top_modules = [\"top\"]\n";
        let offset = toml.find("top").unwrap();

        assert!(toml_manifest_path_at_offset(toml, offset).is_none());
    }
}
