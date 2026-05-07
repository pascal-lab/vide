use base_db::source_db::{SourceDb, SourceRootDb};
use ide_db::root_db::RootDb;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub code: u16,
    pub subsystem: u16,
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
    let mut diagnostics = Vec::new();

    for file_id in source_root.iter() {
        diagnostics.extend(parse_diagnostics(db, file_id));
    }

    diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().map(
        |(diag_file_id, diag)| Diagnostic {
            file_id: *diag_file_id,
            code: diag.code,
            subsystem: diag.subsystem,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        },
    ));

    diagnostics
}

pub(crate) fn source_root_file_ids(db: &RootDb, file_id: FileId) -> Vec<FileId> {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).iter().collect()
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
    use base_db::{change::Change, source_db::SourceRootDb, source_root::SourceRoot};
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::lines::LineEnding;
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
}
