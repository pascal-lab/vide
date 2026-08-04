use std::collections::HashMap;

use base_db::{
    change::Change,
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};
use triomphe::Arc;
use utils::text_edit::{TextRange, TextSize};
use vfs::{ChangedFile, FileId, FileSet, VfsPath};

use crate::{FilePosition, analysis_host::AnalysisHost};

pub(crate) fn normalize_fixture_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(crate) fn setup(text: &str) -> (AnalysisHost, FileId) {
    setup_with_path(text, "/feature.v")
}

pub(crate) fn setup_with_path(text: &str, path: &str) -> (AnalysisHost, FileId) {
    let text = normalize_fixture_text(text);
    let file_id = FileId::from_raw(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_local(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile::create(file_id, text.as_str()));

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id)
}

pub(crate) fn setup_best_effort_with_path(
    text: &str,
    path: &str,
) -> (AnalysisHost, FileId, String) {
    let text = normalize_fixture_text(text);
    let file_id = FileId::from_raw(0);
    let path = VfsPath::new_virtual_path(path.to_string());

    let mut file_set = FileSet::default();
    file_set.insert(file_id, path);
    let root = SourceRoot::new_best_effort_index(file_set);

    let mut change = Change::new();
    change.set_roots(vec![root]);
    change.add_changed_file(ChangedFile::create(file_id, text.as_str()));

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id, text)
}

pub(crate) fn setup_marked(
    text: &str,
) -> (AnalysisHost, FileId, String, HashMap<String, TextSize>) {
    setup_marked_with_path(text, "/feature.v")
}

pub(crate) fn setup_marked_with_path(
    text: &str,
    path: &str,
) -> (AnalysisHost, FileId, String, HashMap<String, TextSize>) {
    let (text, markers) = strip_markers(normalize_fixture_text(text));

    let (host, file_id) = setup_with_path(&text, path);
    (host, file_id, text, markers)
}

pub(crate) type MarkedFile = (FileId, String, HashMap<String, TextSize>);

pub(crate) fn setup_marked_files(files: &[(&str, &str)]) -> (AnalysisHost, Vec<MarkedFile>) {
    let mut file_set = FileSet::default();
    let mut change = Change::new();
    let mut marked_files = Vec::new();

    for (idx, (path, text)) in files.iter().enumerate() {
        let file_id = FileId::from_raw(idx as u32);
        let text = normalize_fixture_text(text);
        let (text, markers) = strip_markers(text);
        file_set.insert(file_id, VfsPath::new_virtual_path((*path).to_owned()));
        change.add_changed_file(ChangedFile::create(file_id, text.as_str()));
        marked_files.push((file_id, text, markers));
    }

    change.set_roots(vec![SourceRoot::new_local(file_set)]);

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, marked_files)
}

pub(crate) fn setup_marked_with_predefines(
    text: &str,
    predefines: Vec<String>,
) -> (AnalysisHost, FileId, String, HashMap<String, TextSize>) {
    let (text, markers) = strip_markers(normalize_fixture_text(text));

    let file_id = FileId::from_raw(0);
    let mut file_set = FileSet::default();
    file_set.insert(file_id, VfsPath::new_virtual_path("/feature.v".to_owned()));

    let mut change = Change::new();
    change.set_roots(vec![SourceRoot::new_local(file_set)]);
    change.set_project_config(Arc::new(ProjectConfig::new(
        vec![Some(CompilationProfileId(0))],
        vec![CompilationProfile {
            source_roots: vec![SourceRootId(0)],
            top_modules: Vec::new(),
            preprocess: PreprocessConfig::with_predefine_strings(predefines, Vec::new()),
        }],
    )));
    change.add_changed_file(ChangedFile::create(file_id, text.as_str()));

    let mut host = AnalysisHost::default();
    host.apply_change(change);
    (host, file_id, text, markers)
}

pub(crate) fn strip_markers(mut text: String) -> (String, HashMap<String, TextSize>) {
    let mut markers = HashMap::new();
    let mut cursor = 0;
    let prefix = "/*marker:";

    while let Some(rel_start) = text[cursor..].find(prefix) {
        let start = cursor + rel_start;
        let name_start = start + prefix.len();
        let Some(rel_end) = text[name_start..].find("*/") else {
            panic!("unterminated marker in fixture");
        };
        let name_end = name_start + rel_end;
        let name = text[name_start..name_end].to_string();
        let end = name_end + 2;
        text.replace_range(start..end, "");
        markers.insert(name, TextSize::from(start as u32));
        cursor = start;
    }

    (text, markers)
}

pub(crate) fn position(
    file_id: FileId,
    markers: &HashMap<String, TextSize>,
    name: &str,
) -> FilePosition {
    FilePosition {
        file_id,
        offset: *markers.get(name).unwrap_or_else(|| panic!("missing marker {name:?}")),
    }
}

pub(crate) fn marked_range(
    markers: &HashMap<String, TextSize>,
    name: &str,
    len: impl Into<TextSize>,
) -> TextRange {
    let start = *markers.get(name).unwrap_or_else(|| panic!("missing marker {name:?}"));
    TextRange::new(start, start + len.into())
}
