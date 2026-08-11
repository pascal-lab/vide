use base_db::project::{Predefine, PreprocessConfig};

use super::*;

pub(crate) fn source_preproc_file_ids(
    db: &dyn PreprocDb,
    file_id: FileId,
    profile_id: Option<CompilationProfileId>,
    trace: &Trace,
    options: &SyntaxTreeOptions,
    preprocess: &PreprocessConfig,
) -> Result<PreprocSourceMap, SourcePreprocQueryError> {
    let mut source_map = PreprocSourceMap::default();
    let path_file_ids = path_file_ids(db);
    let root_source = PreprocSourceId::from(trace.root_buffer_id);
    source_map.insert_real_file(root_source, file_id, db.file_text(file_id).len());
    let include_buffer_texts = include_buffer_texts_by_path(options);
    let predefine_sources = trace
        .source_buffers
        .iter()
        .filter(|source| source.origin == SourceBufferOrigin::Predefine)
        .map(|source| (PreprocSourceId::from(source.buffer_id), source.text.as_deref()))
        .collect::<Vec<_>>();
    let predefine_map =
        PredefineVirtualMapping::new(db, profile_id, &preprocess.predefines, predefine_sources);

    for source in &trace.source_buffers {
        let source_id = PreprocSourceId::from(source.buffer_id);
        if source_id == root_source {
            source_map.insert_real_file(source_id, file_id, db.file_text(file_id).len());
            continue;
        }

        match source.origin {
            SourceBufferOrigin::Source => {
                if let Some(mapped_file_id) = path_file_ids.get(&source.path) {
                    source_map.insert_real_file(
                        source_id,
                        mapped_file_id,
                        db.file_text(mapped_file_id).len(),
                    );
                    continue;
                }

                if let Some(text) = include_buffer_texts.get(&source.path) {
                    let path =
                        preproc_virtual_include_buffer_path(profile_id, source_id, &source.path);
                    let file_id = materialized_preproc_virtual_file_id(db, &path);
                    source_map.insert_virtual_file(
                        source_id,
                        file_id,
                        path,
                        PreprocVirtualOrigin::ExternalIncludeBuffer { source: source_id },
                        text.len(),
                    );
                    continue;
                }

                source_map.insert_unmapped(
                    source_id,
                    SourcePreprocUnavailable::DetachedSource { source: source_id },
                );
            }
            SourceBufferOrigin::Predefine => {
                if let Some(entry) = predefine_map.entry(source_id) {
                    let manifest_source = match entry.manifest_source(db, &path_file_ids) {
                        Ok(manifest_source) => manifest_source,
                        Err(reason) => {
                            source_map.insert_unmapped(source_id, reason);
                            continue;
                        }
                    };
                    source_map.insert_virtual_file_with_offset(
                        source_id,
                        predefine_map.file_id,
                        predefine_map.path.clone(),
                        PreprocVirtualOrigin::Predefines { profile: profile_id },
                        predefine_map.text_len,
                        entry.range_offset,
                    );
                    if let Some(manifest_source) = manifest_source {
                        source_map.insert_predefine_manifest_source(source_id, manifest_source);
                    }
                } else if let Some(reason) = predefine_map.unavailable_reason(source_id) {
                    source_map.insert_unmapped(source_id, reason.clone());
                } else {
                    source_map.insert_unmapped(
                        source_id,
                        SourcePreprocUnavailable::DetachedSource { source: source_id },
                    );
                }
            }
        }
    }

    Ok(source_map)
}

/// Returns the editable macro-name span inside a manifest `defines` value.
///
/// `PredefineSource::range` deliberately covers the complete TOML string so
/// the source-map verification can prove that the configured value still
/// matches the manifest. IDE macro operations need the narrower name span,
/// though, otherwise renaming `FOO=1` would replace the value and the `=1`
/// assignment as well. The returned range is available only when the source
/// spelling has an unambiguous byte mapping and the name follows the manifest
/// macro grammar.
pub fn manifest_predefine_name_range_in_text(text: &str, range: TextRange) -> Option<TextRange> {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let source = toml_parser::Source::new(text);
    let token = source.lex().find(|token| {
        token.span() == toml_parser::Span::new_unchecked(start, end)
            && matches!(
                token.kind(),
                toml_parser::lexer::TokenKind::LiteralString
                    | toml_parser::lexer::TokenKind::BasicString
            )
    })?;
    let raw_text = source.get(token.span())?;
    let raw =
        toml_parser::Raw::new_unchecked(raw_text.as_str(), token.kind().encoding(), token.span());
    let mut decoded = String::new();
    let mut errors = Vec::new();
    if raw.decode_scalar(&mut decoded, &mut errors) != toml_parser::decoder::ScalarKind::String
        || !errors.is_empty()
    {
        return None;
    }
    let raw_content = text.get(start + 1..end.checked_sub(1)?)?;
    let name_len = manifest_macro_name_len(&decoded)?;
    let decoded_name = decoded.get(..name_len)?;
    let quote = raw_text.as_str().get(..1)?;
    let raw_name_len = raw_content
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(raw_content.len()))
        .find(|&candidate_end| {
            let remainder = &raw_content[candidate_end..];
            if !remainder.is_empty() && !remainder.starts_with('=') {
                return false;
            }

            let mut candidate = String::with_capacity(candidate_end + 2);
            candidate.push_str(quote);
            candidate.push_str(&raw_content[..candidate_end]);
            candidate.push_str(quote);
            let candidate_span = toml_parser::Span::new_unchecked(0, candidate.len());
            let candidate_raw = toml_parser::Raw::new_unchecked(
                &candidate,
                token.kind().encoding(),
                candidate_span,
            );
            let mut candidate_decoded = String::new();
            let mut candidate_errors = Vec::new();
            candidate_raw.decode_scalar(&mut candidate_decoded, &mut candidate_errors)
                == toml_parser::decoder::ScalarKind::String
                && candidate_errors.is_empty()
                && candidate_decoded == decoded_name
        })?;

    Some(TextRange::new(
        TextSize::from(u32::try_from(start.checked_add(1)?).ok()?),
        TextSize::from(u32::try_from(start.checked_add(1)?.checked_add(raw_name_len)?).ok()?),
    ))
}

fn manifest_macro_name_len(content: &str) -> Option<usize> {
    let first = content.as_bytes().first().copied()?;
    let name_len = if first == b'\\' {
        let terminator = content[1..].find(char::is_whitespace)? + 1;
        terminator + content[terminator..].chars().next()?.len_utf8()
    } else if first.is_ascii_alphabetic() || first == b'_' {
        content
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
            .count()
    } else {
        return None;
    };

    let rest = &content[name_len..];
    if rest.is_empty() || rest.starts_with('=') {
        return Some(name_len);
    }
    None
}

pub(crate) fn manifest_predefine_name_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    range: TextRange,
) -> Option<TextRange> {
    manifest_predefine_name_range_in_text(db.file_text(file_id).as_ref(), range)
}

pub fn preproc_virtual_predefines_path(profile_id: Option<CompilationProfileId>) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/predefines.sv",
        profile_path_segment(profile_id)
    ))
}

fn preproc_virtual_include_buffer_path(
    profile_id: Option<CompilationProfileId>,
    source_id: PreprocSourceId,
    source_path: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/include-buffer/{}/{}.svh",
        profile_path_segment(profile_id),
        source_id.raw(),
        source_basename(source_path)
    ))
}

fn profile_path_segment(profile_id: Option<CompilationProfileId>) -> String {
    profile_id
        .map(|profile_id| format!("profile-{}", profile_id.0))
        .unwrap_or_else(|| "default".to_owned())
}

fn source_basename(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("buffer");
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    sanitize_path_segment(stem)
}

fn sanitize_path_segment(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => out.push(ch),
            _ => out.push('_'),
        }
    }
    if out.is_empty() { "unnamed".to_owned() } else { out }
}

fn include_buffer_texts_by_path(options: &SyntaxTreeOptions) -> FxHashMap<String, String> {
    options
        .include_buffers
        .iter()
        .map(|buffer| (buffer.path.clone(), buffer.text.clone()))
        .collect()
}

pub(crate) fn materialized_predefine_text(predefine: &str) -> String {
    let mut definition = predefine.to_owned();
    if let Some(index) = definition.find('=') {
        definition.replace_range(index..index + 1, " ");
    } else {
        definition.push_str(" 1");
    }
    format!("`define {definition}\n")
}

/// Maps slang predefine trace buffers back to configured predefines.
///
/// Slang reports predefine buffers as `(path, text)` with no identity link to
/// the config, so buffers are matched by their materialized text against the
/// configured predefines. All matched buffers share one virtual display file
/// (`predefines.sv`) that concatenates the materialized definitions.
struct PredefineVirtualMapping {
    entries: FxHashMap<PreprocSourceId, PredefineVirtualEntry>,
    unavailable: FxHashMap<PreprocSourceId, SourcePreprocUnavailable>,
    /// Shared virtual display file for every matched predefine buffer.
    file_id: Option<FileId>,
    path: VfsPath,
    text_len: usize,
}

struct PredefineVirtualEntry {
    source: PreprocSourceId,
    range_offset: usize,
    predefine: Predefine,
}

impl PredefineVirtualMapping {
    fn new(
        db: &dyn PreprocDb,
        profile_id: Option<CompilationProfileId>,
        predefines: &[Predefine],
        sources: Vec<(PreprocSourceId, Option<&str>)>,
    ) -> Self {
        let texts = predefines
            .iter()
            .map(|predefine| materialized_predefine_text(predefine.as_str()))
            .collect::<Vec<_>>();
        let text_len = texts.iter().map(String::len).sum();
        let path = preproc_virtual_predefines_path(profile_id);
        let file_id = materialized_preproc_virtual_file_id(db, &path);
        let mut range_offset = 0usize;
        let mut configs = Vec::new();
        for (index, predefine) in predefines.iter().enumerate() {
            let text = &texts[index];
            if let Some(name) = materialized_predefine_name(text) {
                configs.push((text.clone(), name, range_offset, predefine.clone()));
            }
            range_offset += text.len();
        }

        // Later configs win over earlier ones with identical materialized
        // text; `rev()` makes the final iteration order config order.
        let mut config_indexes_by_text = FxHashMap::<String, Vec<usize>>::default();
        for (index, (text, _, _, _)) in configs.iter().enumerate().rev() {
            config_indexes_by_text.entry(text.clone()).or_default().push(index);
        }

        let mut entries = FxHashMap::default();
        let mut unavailable = FxHashMap::default();
        for (source, source_text) in sources {
            let Some(source_text) = source_text else {
                unavailable.insert(
                    source,
                    SourcePreprocUnavailable::MissingPredefineSourceText { source },
                );
                continue;
            };
            let Some(config_index) = config_indexes_by_text.get_mut(source_text).and_then(Vec::pop)
            else {
                unavailable
                    .insert(source, SourcePreprocUnavailable::UnverifiedPredefineSource { source });
                continue;
            };
            let (_, name, entry_range_offset, predefine) = &configs[config_index];
            if materialized_predefine_name(source_text).as_ref() != Some(name) {
                unavailable
                    .insert(source, SourcePreprocUnavailable::UnverifiedPredefineSource { source });
                continue;
            }
            entries.insert(
                source,
                PredefineVirtualEntry {
                    source,
                    range_offset: *entry_range_offset,
                    predefine: predefine.clone(),
                },
            );
        }

        Self { entries, unavailable, file_id, path, text_len }
    }

    fn entry(&self, source: PreprocSourceId) -> Option<&PredefineVirtualEntry> {
        self.entries.get(&source)
    }

    fn unavailable_reason(&self, source: PreprocSourceId) -> Option<&SourcePreprocUnavailable> {
        self.unavailable.get(&source)
    }
}

impl PredefineVirtualEntry {
    fn manifest_source(
        &self,
        db: &dyn PreprocDb,
        path_file_ids: &PathIdentityIndex<FileId>,
    ) -> Result<Option<PreprocManifestSource>, SourcePreprocUnavailable> {
        let Some(source) = self.predefine.source.as_ref() else {
            return Ok(None);
        };
        let Some(file_id) = path_file_ids.get_path(source.path.as_path()) else {
            return Err(SourcePreprocUnavailable::UnverifiedPredefineSource {
                source: self.source,
            });
        };
        if !manifest_predefine_source_matches(
            db.file_text(file_id).as_ref(),
            source.range,
            &self.predefine,
        ) {
            return Err(SourcePreprocUnavailable::UnverifiedPredefineSource {
                source: self.source,
            });
        }
        let name_range = manifest_predefine_name_range(db, file_id, source.range)
            .ok_or(SourcePreprocUnavailable::UnverifiedPredefineSource { source: self.source })?;
        Ok(Some(PreprocManifestSource { file_id, range: source.range, name_range }))
    }
}

fn materialized_predefine_name(text: &str) -> Option<SmolStr> {
    let rest = text.trim_start().strip_prefix("`define")?.trim_start();
    let name = rest.split(|ch: char| ch.is_whitespace() || ch == '(').next()?.trim();
    let name = name.strip_prefix('`').unwrap_or(name);
    if name.is_empty() { None } else { Some(SmolStr::new(name)) }
}

fn manifest_predefine_source_matches(text: &str, range: TextRange, predefine: &Predefine) -> bool {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    let Some(raw_source) = text.get(start..end) else {
        return false;
    };
    let Some(source_definition) = decode_manifest_predefine_source(raw_source) else {
        return false;
    };
    source_definition.as_str() == predefine.as_str()
        && crate::preproc::predefine_name(source_definition.as_str())
            == crate::preproc::predefine_name(predefine.as_str())
}

fn decode_manifest_predefine_source(text: &str) -> Option<String> {
    let document = format!("value = {}", text.trim());
    toml::from_str::<toml::Value>(&document)
        .ok()
        .and_then(|document| document.get("value").and_then(toml::Value::as_str).map(str::to_owned))
}

fn materialized_preproc_virtual_file_id(db: &dyn PreprocDb, path: &VfsPath) -> Option<FileId> {
    file_id_for_vfs_path(db, path)
}

fn file_id_for_vfs_path(db: &dyn PreprocDb, path: &VfsPath) -> Option<FileId> {
    for file_id in db.files().iter().copied() {
        let source_root_id = db.source_root_id(file_id);
        let source_root = db.source_root(source_root_id);
        if source_root.path_for_file(&file_id) == Some(path) {
            return Some(file_id);
        }
    }
    None
}

/// Shift a range by `range_offset`. This and [`unshift_text_size`] are the
/// **single** authoritative range-shift primitives for slang-buffer offsets
/// -> user-file offsets: both the buffer->file map (`PreprocSourceMap`,
/// `source_map/mapping.rs`) and, transitively, the expansion token map
/// (`macro_file/source_map.rs`) map ranges through
/// `PreprocSourceMap::map_range`. Range arithmetic lives here and nowhere else
/// so the two layers cannot drift apart.
pub(in crate::source_db) fn shift_text_range(range: TextRange, offset: usize) -> Option<TextRange> {
    let start = usize::from(range.start()).checked_add(offset)?;
    let end = usize::from(range.end()).checked_add(offset)?;
    Some(TextRange::new(
        TextSize::from(u32::try_from(start).ok()?),
        TextSize::from(u32::try_from(end).ok()?),
    ))
}

pub(in crate::source_db) fn unshift_text_size(
    offset: TextSize,
    range_offset: usize,
) -> Option<TextSize> {
    let offset = usize::from(offset).checked_sub(range_offset)?;
    Some(TextSize::from(u32::try_from(offset).ok()?))
}

#[cfg(test)]
mod tests {
    use utils::line_index::{TextRange, TextSize};

    use super::manifest_predefine_name_range_in_text;

    fn range(text: &str) -> TextRange {
        TextRange::new(TextSize::default(), TextSize::of(text))
    }

    #[test]
    fn macro_name_range_requires_manifest_macro_grammar() {
        let text = "\"FEATURE=1\"";
        assert_eq!(
            manifest_predefine_name_range_in_text(text, range(text)),
            Some(TextRange::new(TextSize::from(1), TextSize::from(8)))
        );
        assert_eq!(
            manifest_predefine_name_range_in_text("\"FEATURE\"", range("\"FEATURE\"")),
            Some(TextRange::new(TextSize::from(1), TextSize::from(8)))
        );
        assert_eq!(
            manifest_predefine_name_range_in_text(
                "\"FEATURE-NAME=1\"",
                range("\"FEATURE-NAME=1\"")
            ),
            None
        );
        assert_eq!(
            manifest_predefine_name_range_in_text(
                r#""FEATURE=\"hello\"""#,
                range(r#""FEATURE=\"hello\"""#)
            ),
            Some(TextRange::new(TextSize::from(1), TextSize::from(8)))
        );
    }
}
