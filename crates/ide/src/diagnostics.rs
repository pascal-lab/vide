use base_db::source_db::SourceRootDb;
use ide_db::root_db::RootDb;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    SlangParse,
    SlangSemantic,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub code: u16,
    pub subsystem: u16,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub source: DiagnosticSource,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

pub(crate) fn parse_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    db.parse_diagnostics(file_id)
        .iter()
        .map(|diag| Diagnostic {
            file_id,
            code: diag.code,
            subsystem: diag.subsystem,
            name: diag.name.clone(),
            option_name: diag.option_name.clone(),
            groups: diag.groups.clone(),
            source: DiagnosticSource::SlangParse,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        })
        .collect()
}

pub(crate) fn diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = parse_diagnostics(db, file_id);

    diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().filter_map(
        |(diag_file_id, diag)| {
            (*diag_file_id == file_id).then_some(Diagnostic {
                file_id: *diag_file_id,
                code: diag.code,
                subsystem: diag.subsystem,
                name: diag.name.clone(),
                option_name: diag.option_name.clone(),
                groups: diag.groups.clone(),
                source: DiagnosticSource::SlangSemantic,
                range: to_text_range(diag),
                severity: diag.severity,
                message: diag.message.clone(),
            })
        },
    ));

    diagnostics
}

pub(crate) fn source_root_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let source_root_id = db.source_root_id(file_id);
    let source_root = db.source_root(source_root_id);
    if source_root.is_ignored() {
        return Vec::new();
    }
    let mut diagnostics = Vec::new();

    for file_id in source_root.iter() {
        diagnostics.extend(parse_diagnostics(db, file_id));
    }

    diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().map(
        |(diag_file_id, diag)| Diagnostic {
            file_id: *diag_file_id,
            code: diag.code,
            subsystem: diag.subsystem,
            name: diag.name.clone(),
            option_name: diag.option_name.clone(),
            groups: diag.groups.clone(),
            source: DiagnosticSource::SlangSemantic,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        },
    ));

    diagnostics
}

pub(crate) fn source_root_file_ids(db: &RootDb, file_id: FileId) -> Vec<FileId> {
    let source_root_id = db.source_root_id(file_id);
    let source_root = db.source_root(source_root_id);
    if source_root.is_ignored() { vec![file_id] } else { source_root.iter().collect() }
}

fn to_text_range(diag: &SyntaxDiagnostic) -> TextRange {
    fn to_text_size(value: usize) -> TextSize {
        let raw = u32::try_from(value).unwrap_or(u32::MAX);
        TextSize::new(raw)
    }

    if let Some(range) = diag.primary_range.as_ref() {
        TextRange::new(to_text_size(range.start), to_text_size(range.end))
    } else if let Some(offset) = diag.location {
        let pos = to_text_size(offset);
        TextRange::new(pos, pos)
    } else {
        TextRange::empty(TextSize::new(0))
    }
}

#[cfg(test)]
mod tests {
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_db::SourceRootDb,
        source_root::{SourceRoot, SourceRootId},
    };
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::{lines::LineEnding, paths::AbsPathBuf};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::diagnostics;

    fn db_with_files(files: &[(&str, &str)]) -> RootDb {
        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        let mut change = Change::new();

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            let path = VfsPath::new_virtual_path((*path).to_owned());
            file_set.insert(file_id, path);
            change.add_changed_file(ChangedFile {
                file_id,
                change_kind: ChangeKind::Create(Arc::from(*text), LineEnding::Unix),
            });
        }

        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);
        db
    }

    #[test]
    fn semantic_diagnostics_include_other_workspace_files() {
        let db = db_with_files(&[
            ("/child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("/top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ]);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
            "expected semantic diagnostic from module declared in another file: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "document diagnostics should only include diagnostics attributed to the requested file: {diagnostics:?}"
        );
        assert!(
            db.semantic_diagnostics(FileId(0)).is_empty(),
            "child file should not receive diagnostics that belong to top.sv"
        );
    }

    #[test]
    fn semantic_diagnostics_map_include_header_files() {
        let root =
            std::env::temp_dir().join(format!("vizsla-diagnostics-include-{}", std::process::id()));
        let root = AbsPathBuf::assert_utf8(root);
        let top_path = root.join("top.sv");
        let header_path = root.join("defs.vh");

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(top_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(header_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(
                Arc::from("module top;\n`include \"defs.vh\"\nendmodule\n"),
                LineEnding::Unix,
            ),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(
                Arc::from("logic value = missing_name;\n"),
                LineEnding::Unix,
            ),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig { predefines: Vec::new(), include_dirs: vec![root] },
            }],
        )));
        db.apply_change(change);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("missing_name")),
            "expected semantic diagnostic in included header: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "header diagnostics should be attributed to the header file: {diagnostics:?}"
        );
    }
}
