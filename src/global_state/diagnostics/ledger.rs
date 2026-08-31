//! Anchored diagnostic facts.
//!
//! Profile slang diagnostics and qihe results hang on [`ide::anchor::Anchor`]
//! (a `SourceAstId` in this file). Edit → reproject + freshness, not drop.

use std::sync::Arc as StdArc;

use base_db::analysis_snapshot::AnalysisSnapshotId;
use parking_lot::{Mutex, MutexGuard};
use rustc_hash::{FxHashMap, FxHashSet};
use vfs::FileId;

use crate::i18n::{I18n, keys};

#[derive(Debug, Clone)]
pub(crate) struct AnchoredDiagnostic<T> {
    pub ast_id: Option<hir_def::ast_id_map::SourceAstId>,
    pub diagnostic: T,
}

#[derive(Debug, Clone)]
pub(crate) struct FileDiagnosticState<T> {
    pub captured_snapshot: AnalysisSnapshotId,
    pub generation: u64,
    pub items: Vec<AnchoredDiagnostic<T>>,
}

#[derive(Clone)]
pub(crate) struct DiagnosticLedger<T> {
    states: StdArc<Mutex<FxHashMap<FileId, FileDiagnosticState<T>>>>,
}

impl<T> Default for DiagnosticLedger<T> {
    fn default() -> Self {
        Self { states: StdArc::new(Mutex::new(FxHashMap::default())) }
    }
}

impl<T> DiagnosticLedger<T> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, FxHashMap<FileId, FileDiagnosticState<T>>> {
        self.states.lock()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    pub(crate) fn remove_deleted(&self, files: &FxHashSet<FileId>) {
        if files.is_empty() {
            return;
        }
        let mut states = self.lock();
        for file_id in files {
            states.remove(file_id);
        }
    }
}

impl<T: Clone> DiagnosticLedger<T> {
    /// Replace facts for `by_file` keys and clear files that previously had
    /// facts but are absent from this commit.
    pub(crate) fn replace(
        &self,
        mut by_file: FxHashMap<FileId, Vec<T>>,
        captured_snapshot: AnalysisSnapshotId,
    ) -> FxHashSet<FileId> {
        let mut cache = self.lock();
        let mut changed_files = cache
            .iter()
            .filter_map(|(&file_id, state)| (!state.items.is_empty()).then_some(file_id))
            .collect::<FxHashSet<_>>();
        changed_files.extend(by_file.keys().copied());

        for file_id in &changed_files {
            let diagnostics = by_file.remove(file_id).unwrap_or_default();
            let generation =
                cache.get(file_id).map_or(1, |state| state.generation.saturating_add(1));
            let items = diagnostics
                .into_iter()
                .map(|diagnostic| AnchoredDiagnostic { ast_id: None, diagnostic })
                .collect();
            cache.insert(*file_id, FileDiagnosticState { captured_snapshot, generation, items });
        }

        changed_files
    }
}

pub(crate) fn edits_ago(current: AnalysisSnapshotId, captured: AnalysisSnapshotId) -> u64 {
    current.get().saturating_sub(captured.get())
}

pub(crate) fn freshness_note(i18n: I18n, edits_ago: u64) -> Option<String> {
    (edits_ago > 0)
        .then(|| i18n.format(keys::DIAGNOSTIC_BASED_ON_EDITS, [("n", edits_ago.to_string())]))
}

pub(crate) fn project_definition_range(
    analysis: &ide::analysis::AnalysisSnapshot,
    file_id: FileId,
    ast_id: hir_def::ast_id_map::SourceAstId,
) -> Option<utils::line_index::TextRange> {
    analysis
        .project_anchor(ide::anchor::Anchor::Definition { file: file_id, ast_id })
        .ok()
        .flatten()
        .map(|origin| origin.range)
}

pub(crate) fn append_freshness_note(message: &mut String, note: &str) {
    if !message.contains(note) {
        *message = format!("{message}\n{note}");
    }
}

/// One elaborated instance captured from a profile compilation.
#[derive(Debug, Clone)]
pub(crate) struct AnchoredInstance {
    pub path: ide::hier::HierPath,
    pub file: FileId,
    pub ast_id: Option<hir_def::ast_id_map::SourceAstId>,
}

#[derive(Clone, Default)]
pub(crate) struct InstanceLedger {
    inner: StdArc<Mutex<InstanceLedgerInner>>,
}

#[derive(Clone, Default)]
struct InstanceLedgerInner {
    captured_snapshot: AnalysisSnapshotId,
    items: Vec<AnchoredInstance>,
}

impl InstanceLedger {
    pub(crate) fn replace(
        &self,
        captured_snapshot: AnalysisSnapshotId,
        items: Vec<AnchoredInstance>,
    ) {
        *self.inner.lock() = InstanceLedgerInner { captured_snapshot, items };
    }

    pub(crate) fn projected(
        &self,
        analysis: &ide::analysis::AnalysisSnapshot,
    ) -> Vec<(ide::hier::HierPath, FileId, utils::line_index::TextRange)> {
        let inner = self.inner.lock().clone();
        inner
            .items
            .into_iter()
            .filter_map(|item| {
                let range = item
                    .ast_id
                    .and_then(|ast_id| project_definition_range(analysis, item.file, ast_id))?;
                Some((item.path, item.file, range))
            })
            .collect()
    }

    pub(crate) fn edits_ago(&self, current: AnalysisSnapshotId) -> u64 {
        edits_ago(current, self.inner.lock().captured_snapshot)
    }
}
