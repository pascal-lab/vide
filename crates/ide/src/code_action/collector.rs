use utils::text_edit::TextRange;
use vfs::FileId;

use super::{CodeAction, CodeActionId};
use crate::{
    code_action::CodeActionResolveStrategy, diagnostics::Diagnostic,
    source_change::SourceChangeBuilder,
};

pub(crate) struct CodeActionCollector<'a> {
    file: FileId,
    resolve_strategy: CodeActionResolveStrategy,
    diagnostics: &'a [Diagnostic],
    buf: Vec<CodeAction>,
    /// Per-request, monotonically increasing occurrence counter. Each action
    /// gets a distinct `ordinal` so that `(name, ordinal)` uniquely identifies
    /// an occurrence even when a handler offers several actions with the same
    /// name (e.g. `convert_literal_base`'s per-target-base variants). The
    /// engine is deterministic, so the same ordinal is recomputed on resolve.
    next_ordinal: u32,
}

impl<'a> CodeActionCollector<'a> {
    pub(super) fn new(
        file: FileId,
        resolve_strategy: CodeActionResolveStrategy,
        diagnostics: &'a [Diagnostic],
    ) -> Self {
        Self { file, resolve_strategy, diagnostics, buf: Vec::new(), next_ordinal: 0 }
    }

    pub(crate) fn add(
        &mut self,
        id: CodeActionId,
        label: impl Into<String>,
        target: TextRange,
        f: impl FnOnce(&mut SourceChangeBuilder),
    ) -> Option<()> {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;

        let source_change = if self.resolve_strategy.should_resolve(id) {
            let mut builder = SourceChangeBuilder::new(self.file);
            f(&mut builder);
            Some(builder.finish())
        } else {
            None
        };

        self.buf.push(CodeAction {
            id,
            ordinal,
            label: label.into(),
            target,
            source_change,
            diagnostics: Vec::new(),
        });
        Some(())
    }

    /// Attaches the diagnostics each action repairs and classifies quick
    /// fixes, then sorts by target size.
    ///
    /// Only diagnostics that the action's `RepairKind` matches **and** whose
    /// range overlaps the action's target are attached, so an action never
    /// claims a diagnostic for an unrelated spot that merely shares the
    /// request range.
    pub(super) fn finish(mut self) -> Vec<CodeAction> {
        for action in &mut self.buf {
            let Some(repair) = action.id.repair else {
                continue;
            };
            let matched: Vec<Diagnostic> = self
                .diagnostics
                .iter()
                .filter(|diag| repair.matches(diag) && ranges_overlap(diag.range, action.target))
                .cloned()
                .collect();
            if !matched.is_empty() {
                action.id.kind = super::CodeActionKind::QuickFix;
                action.diagnostics = matched;
            }
        }
        self.buf.sort_by_key(|assist| assist.target.len());
        self.buf
    }
}

/// Point-aware range overlap, mirroring the request-range matching in the LSP
/// handler: two empty ranges overlap when they are the same point, and a point
/// overlaps a range when it lies on it.
fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    if a.is_empty() && b.is_empty() {
        a.start() == b.start()
    } else if a.is_empty() {
        let p = a.start();
        b.start() <= p && p <= b.end()
    } else if b.is_empty() {
        let p = b.start();
        a.start() <= p && p <= a.end()
    } else {
        a.start() < b.end() && b.start() < a.end()
    }
}
