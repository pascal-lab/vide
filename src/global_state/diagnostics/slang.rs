//! Profile slang diagnostics on the anchored ledger.

use base_db::analysis_snapshot::AnalysisSnapshotId;
use vfs::FileId;

use super::{DiagnosticLedger, edits_ago, project_definition_range};

#[derive(Clone, Default)]
pub(crate) struct SlangDiagnostics {
    pub ledger: DiagnosticLedger<ide::diagnostics::Diagnostic>,
}

impl SlangDiagnostics {
    pub(crate) fn new() -> Self {
        Self { ledger: DiagnosticLedger::new() }
    }

    pub(crate) fn replace(
        &self,
        by_file: rustc_hash::FxHashMap<FileId, Vec<ide::diagnostics::Diagnostic>>,
        captured_snapshot: AnalysisSnapshotId,
    ) {
        self.ledger.replace(by_file, captured_snapshot);
    }

    pub(crate) fn ide_diagnostics(
        &self,
        file_id: FileId,
        current_snapshot: AnalysisSnapshotId,
        analysis: &ide::analysis::AnalysisSnapshot,
    ) -> Vec<ide::diagnostics::Diagnostic> {
        let mut cache = self.ledger.lock();
        let Some(state) = cache.get_mut(&file_id) else {
            return Vec::new();
        };
        for item in &mut state.items {
            if item.ast_id.is_none() {
                item.ast_id =
                    analysis.ast_id_at_range(file_id, item.diagnostic.range).ok().flatten();
            }
        }
        let items = state.items.clone();
        drop(cache);
        let _ = current_snapshot;
        items
            .into_iter()
            .map(|item| {
                let mut diagnostic = item.diagnostic;
                if let Some(ast_id) = item.ast_id
                    && let Some(range) = project_definition_range(analysis, file_id, ast_id)
                {
                    diagnostic.range = range;
                }
                diagnostic
            })
            .collect()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ledger.is_empty()
    }

    pub(crate) fn has_file(&self, file_id: FileId) -> bool {
        self.ledger.lock().contains_key(&file_id)
    }

    pub(crate) fn edits_ago(&self, file_id: FileId, current_snapshot: AnalysisSnapshotId) -> u64 {
        self.ledger
            .lock()
            .get(&file_id)
            .map(|state| edits_ago(current_snapshot, state.captured_snapshot))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use ide::analysis_host::AnalysisHost;
    use rustc_hash::FxHashMap;
    use syntax::diagnostics::DiagnosticSeverity;
    use utils::line_index::{TextRange, TextSize};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{global_state::diagnostics::freshness_note, i18n::I18n};

    fn host_with(text: &str) -> (AnalysisHost, FileId) {
        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, text));
        let mut host = AnalysisHost::default();
        host.apply_change(change);
        (host, file_id)
    }

    fn slang_diag(
        file_id: FileId,
        range: TextRange,
        message: &str,
    ) -> ide::diagnostics::Diagnostic {
        ide::diagnostics::Diagnostic {
            file_id,
            code: 1,
            subsystem: 0,
            name: "test".to_owned(),
            option_name: None,
            groups: Vec::new(),
            source: ide::diagnostics::DiagnosticSource::SlangSemantic,
            range,
            severity: DiagnosticSeverity::Warning,
            message: message.to_owned(),
            args: Vec::new(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn slang_diagnostics_reproject_after_an_insert_and_keep_a_freshness_label() {
        let src = "module top; wire x; endmodule\n";
        let (mut host, file_id) = host_with(src);
        let offset = src.find("x").expect("x") as u32;
        let range = TextRange::new(TextSize::from(offset), TextSize::from(offset + 1));
        let captured = {
            let analysis = host.make_analysis();
            let ast_id = analysis.ast_id_at_range(file_id, range).unwrap().expect("ast id");
            let store = SlangDiagnostics::new();
            store.replace(
                FxHashMap::from_iter([(file_id, vec![slang_diag(file_id, range, "width")])]),
                analysis.snapshot_id(),
            );
            store.ledger.lock().get_mut(&file_id).unwrap().items[0].ast_id = Some(ast_id);
            (store, analysis.snapshot_id())
        };

        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(file_id, format!("// header\n{src}").as_str()));
        host.apply_change(change);

        let analysis = host.make_analysis();
        let store = captured.0;
        let projected = store.ide_diagnostics(file_id, analysis.snapshot_id(), &analysis);
        assert_eq!(projected.len(), 1);
        assert!(
            projected[0].range.start() > range.start(),
            "insert before the name must shift the diagnostic: {:?}",
            projected[0].range
        );
        assert_eq!(store.edits_ago(file_id, analysis.snapshot_id()), 1);
        let note = freshness_note(I18n::default(), 1).expect("stale facts are labeled");
        assert!(note.contains("edit"), "{note}");
        let _ = captured.1;
    }

    #[test]
    fn instance_sites_reproject_after_an_insert() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let (mut host, file_id) = host_with(src);
        let offset = src.find("u0").expect("u0") as u32;
        let range = TextRange::new(TextSize::from(offset), TextSize::from(offset + 2));
        let ledger = crate::global_state::diagnostics::InstanceLedger::default();
        {
            let analysis = host.make_analysis();
            let ast_id = analysis.ast_id_at_range(file_id, range).unwrap().expect("ast id");
            ledger.replace(
                analysis.snapshot_id(),
                vec![crate::global_state::diagnostics::AnchoredInstance {
                    path: ide::hier::HierPath::new("top.u0"),
                    file: file_id,
                    ast_id: Some(ast_id),
                }],
            );
        }
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(file_id, format!("// header\n{src}").as_str()));
        host.apply_change(change);
        let analysis = host.make_analysis();
        let projected = ledger.projected(&analysis);
        assert_eq!(projected.len(), 1);
        assert!(
            projected[0].2.start() > range.start(),
            "instance site must reproject: {:?}",
            projected[0].2
        );
        assert_eq!(ledger.edits_ago(analysis.snapshot_id()), 1);
    }
}
