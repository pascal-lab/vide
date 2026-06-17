use utils::text_edit::TextRange;
use vfs::FileId;

pub(super) fn diagnostic_preproc_target_file_range(
    provenance: &hir::preproc::DiagnosticProvenance,
) -> Option<(FileId, TextRange)> {
    match provenance {
        hir::preproc::DiagnosticProvenance::SourceToken { file_id, range }
        | hir::preproc::DiagnosticProvenance::MacroBody { file_id, range, .. }
        | hir::preproc::DiagnosticProvenance::MacroArgument { file_id, range, .. }
        | hir::preproc::DiagnosticProvenance::VirtualExpansion { file_id, range } => {
            Some((*file_id, *range))
        }
        hir::preproc::DiagnosticProvenance::Builtin { call, .. } => {
            Some((call.file_id, call.range))
        }
        hir::preproc::DiagnosticProvenance::Unavailable(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use utils::text_edit::{TextRange, TextSize};
    use vfs::FileId;

    use super::diagnostic_preproc_target_file_range;

    #[test]
    fn diagnostic_target_accepts_materialized_virtual_expansion() {
        let file_id = FileId(7);
        let range = TextRange::new(TextSize::from(0), TextSize::from(5));
        let provenance = hir::preproc::DiagnosticProvenance::VirtualExpansion { file_id, range };

        assert_eq!(diagnostic_preproc_target_file_range(&provenance), Some((file_id, range)));
    }

    #[test]
    fn diagnostic_target_rejects_unavailable_target() {
        let provenance = hir::preproc::DiagnosticProvenance::Unavailable(
            hir::preproc::PreprocUnavailable::AmbiguousDiagnosticProvenance { targets: 2 },
        );

        assert_eq!(diagnostic_preproc_target_file_range(&provenance), None);
    }
}
