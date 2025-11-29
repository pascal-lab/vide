use base_db::source_db::SourceDb;
use ide_db::root_db::RootDb;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

#[derive(Debug, Clone)]
pub struct Diagnostic {
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
            code: diag.code,
            subsystem: diag.subsystem,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        })
        .collect()
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
