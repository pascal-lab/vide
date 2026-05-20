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

#[derive(Debug, PartialEq, Eq)]
pub struct TomlManifestFieldCompletionContext {
    pub replacement_range: Range<usize>,
    pub existing_fields: Vec<String>,
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

pub fn toml_manifest_field_completion_context(
    text: &str,
    offset: usize,
) -> Option<TomlManifestFieldCompletionContext> {
    let offset = offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let line_end = text[offset..].find('\n').map(|idx| offset + idx).unwrap_or(text.len());
    let replacement_range = top_level_key_replacement_range(text, line_start, offset)?;
    let mut parseable_text =
        String::with_capacity(text.len().saturating_sub(line_end - line_start));
    parseable_text.push_str(&text[..line_start]);
    parseable_text.push_str(&text[line_end..]);
    let existing_fields =
        toml_manifest_fields(&parseable_text).into_iter().map(|field| field.key).collect();

    Some(TomlManifestFieldCompletionContext { replacement_range, existing_fields })
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

fn top_level_key_replacement_range(
    text: &str,
    line_start: usize,
    offset: usize,
) -> Option<Range<usize>> {
    let line_prefix = text.get(line_start..offset)?;
    let replace_start = line_start
        + line_prefix
            .char_indices()
            .rev()
            .find_map(|(idx, ch)| (!is_key_char(ch)).then_some(idx + ch.len_utf8()))
            .unwrap_or(0);
    let before_key = text.get(line_start..replace_start)?;
    before_key.chars().all(char::is_whitespace).then_some(replace_start..offset)
}

fn is_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
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
    fn field_completion_context_filters_existing_fields_outside_current_line() {
        let toml = "sources = []\ninc";
        let offset = toml.len();
        let context = toml_manifest_field_completion_context(toml, offset).unwrap();

        assert_eq!(context.replacement_range, toml.find("inc").unwrap()..offset);
        assert_eq!(context.existing_fields, ["sources"]);
    }

    #[test]
    fn field_completion_context_keeps_current_line_editable() {
        let toml = "sources";
        let context = toml_manifest_field_completion_context(toml, toml.len()).unwrap();

        assert_eq!(context.replacement_range, 0..toml.len());
        assert!(context.existing_fields.is_empty());
    }

    #[test]
    fn field_completion_context_ignores_values_and_comments() {
        let value_toml = "sources = [\"rtl\"]\n";
        let value_offset = value_toml.find("rtl").unwrap();
        assert!(toml_manifest_field_completion_context(value_toml, value_offset).is_none());

        let comment_toml = "# sou";
        assert!(toml_manifest_field_completion_context(comment_toml, comment_toml.len()).is_none());
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
