use super::*;
use crate::base_db::project::{Predefine, PreprocessConfig};

pub(in crate::base_db::source_db) fn source_preproc_file_ids(
    db: &dyn SourceRootDb,
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
        .map(|source| PredefineSourceBuffer {
            source: PreprocSourceId::from(source.buffer_id),
            text: source.text.as_deref(),
        })
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
                        entry.file_id,
                        entry.path.clone(),
                        PreprocVirtualOrigin::Predefines { profile: profile_id },
                        entry.text_len,
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

pub fn preproc_virtual_predefines_path(profile_id: Option<CompilationProfileId>) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/predefines.sv",
        profile_path_segment(profile_id)
    ))
}

pub fn preproc_virtual_builtin_path(
    profile_id: Option<CompilationProfileId>,
    name: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/builtin/{}.sv",
        profile_path_segment(profile_id),
        sanitize_path_segment(name)
    ))
}

pub fn preproc_virtual_speculative_path(
    profile_id: Option<CompilationProfileId>,
    universe: PreprocSpeculativeUniverseId,
    root: &str,
) -> VfsPath {
    VfsPath::new_virtual_path(format!(
        "/__vide/preproc/{}/speculative/{}/{}.sv",
        profile_path_segment(profile_id),
        universe.0,
        sanitize_path_segment(root)
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

pub fn preproc_virtual_predefines_text(predefines: &[Predefine]) -> String {
    let mut text = String::new();
    for predefine in predefines {
        text.push_str(&materialized_predefine_text(predefine.as_str()));
    }
    text
}

pub(in crate::base_db::source_db) fn materialized_predefine_text(predefine: &str) -> String {
    let mut definition = predefine.to_owned();
    if let Some(index) = definition.find('=') {
        definition.replace_range(index..index + 1, " ");
    } else {
        definition.push_str(" 1");
    }
    format!("`define {definition}\n")
}

struct PredefineVirtualMapping {
    entries: FxHashMap<PreprocSourceId, PredefineVirtualEntry>,
    unavailable: FxHashMap<PreprocSourceId, SourcePreprocUnavailable>,
}

struct PredefineVirtualEntry {
    source: PreprocSourceId,
    file_id: Option<FileId>,
    path: VfsPath,
    text_len: usize,
    range_offset: usize,
    predefine: Predefine,
}

struct PredefineSourceBuffer<'a> {
    source: PreprocSourceId,
    text: Option<&'a str>,
}

struct PredefineConfigEntry {
    text: String,
    name: SmolStr,
    range_offset: usize,
    predefine: Predefine,
}

impl PredefineVirtualMapping {
    fn new(
        db: &dyn SourceRootDb,
        profile_id: Option<CompilationProfileId>,
        predefines: &[Predefine],
        sources: Vec<PredefineSourceBuffer<'_>>,
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
                configs.push(PredefineConfigEntry {
                    text: text.clone(),
                    name,
                    range_offset,
                    predefine: predefine.clone(),
                });
            }
            range_offset += text.len();
        }

        let mut config_indexes_by_text = FxHashMap::<String, Vec<usize>>::default();
        for (index, config) in configs.iter().enumerate().rev() {
            config_indexes_by_text.entry(config.text.clone()).or_default().push(index);
        }

        let mut entries = FxHashMap::default();
        let mut unavailable = FxHashMap::default();
        for source in sources {
            let Some(source_text) = source.text else {
                unavailable.insert(
                    source.source,
                    SourcePreprocUnavailable::MissingPredefineSourceText { source: source.source },
                );
                continue;
            };
            let Some(config_index) = config_indexes_by_text.get_mut(source_text).and_then(Vec::pop)
            else {
                unavailable.insert(
                    source.source,
                    SourcePreprocUnavailable::UnverifiedPredefineSource { source: source.source },
                );
                continue;
            };
            let config = &configs[config_index];
            if materialized_predefine_name(source_text).as_ref() != Some(&config.name) {
                unavailable.insert(
                    source.source,
                    SourcePreprocUnavailable::UnverifiedPredefineSource { source: source.source },
                );
                continue;
            }
            entries.insert(
                source.source,
                PredefineVirtualEntry {
                    source: source.source,
                    file_id,
                    path: path.clone(),
                    text_len,
                    range_offset: config.range_offset,
                    predefine: config.predefine.clone(),
                },
            );
        }

        Self { entries, unavailable }
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
        db: &dyn SourceRootDb,
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
        Ok(Some(PreprocManifestSource { file_id, range: source.range }))
    }
}

fn materialized_predefine_name(text: &str) -> Option<SmolStr> {
    let rest = text.trim_start().strip_prefix("`define")?.trim_start();
    let name =
        rest.split(|ch: char| ch.is_whitespace() || ch == '(').next().unwrap_or_default().trim();
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
        && predefine_definition_name(source_definition.as_str())
            == predefine_definition_name(predefine.as_str())
}

fn decode_manifest_predefine_source(text: &str) -> Option<String> {
    let document = format!("value = {}", text.trim());
    toml::from_str::<toml::Value>(&document)
        .ok()
        .and_then(|document| document.get("value").and_then(toml::Value::as_str).map(str::to_owned))
}

fn predefine_definition_name(predefine: &str) -> Option<SmolStr> {
    let name = predefine.split_once('=').map_or(predefine, |(name, _)| name);
    let name = name.trim().strip_prefix('`').unwrap_or(name.trim());
    if name.is_empty() { None } else { Some(SmolStr::new(name)) }
}

fn materialized_preproc_virtual_file_id(db: &dyn SourceRootDb, path: &VfsPath) -> Option<FileId> {
    file_id_for_vfs_path(db, path)
}

fn file_id_for_vfs_path(db: &dyn SourceRootDb, path: &VfsPath) -> Option<FileId> {
    for file_id in db.files().iter().copied() {
        let source_root_id = db.source_root_id(file_id);
        let source_root = db.source_root(source_root_id);
        if source_root.path_for_file(&file_id) == Some(path) {
            return Some(file_id);
        }
    }
    None
}

pub(in crate::base_db::source_db::preproc) fn shift_text_range(
    range: TextRange,
    offset: usize,
) -> Option<TextRange> {
    let start = usize::from(range.start()).checked_add(offset)?;
    let end = usize::from(range.end()).checked_add(offset)?;
    Some(TextRange::new(
        TextSize::from(u32::try_from(start).ok()?),
        TextSize::from(u32::try_from(end).ok()?),
    ))
}

pub(in crate::base_db::source_db::preproc) fn unshift_text_size(
    offset: TextSize,
    range_offset: usize,
) -> Option<TextSize> {
    let offset = usize::from(offset).checked_sub(range_offset)?;
    Some(TextSize::from(u32::try_from(offset).ok()?))
}
