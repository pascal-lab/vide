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
}

impl<'a> CodeActionCollector<'a> {
    pub(super) fn new(
        file: FileId,
        resolve_strategy: CodeActionResolveStrategy,
        diagnostics: &'a [Diagnostic],
    ) -> Self {
        Self { file, resolve_strategy, diagnostics, buf: Vec::new() }
    }

    pub(crate) fn add(
        &mut self,
        id: CodeActionId,
        label: impl Into<String>,
        target: TextRange,
        f: impl FnOnce(&mut SourceChangeBuilder),
    ) -> Option<()> {
        let source_change = if self.resolve_strategy.should_resolve(id) {
            let mut builder = SourceChangeBuilder::new(self.file);
            f(&mut builder);
            Some(builder.finish())
        } else {
            None
        };

        self.buf.push(CodeAction {
            id,
            label: label.into(),
            target,
            source_change,
            diagnostics: Vec::new(),
        });
        Some(())
    }

    /// Attaches the diagnostics each action repairs and classifies quick
    /// fixes, then sorts by target size.
    pub(super) fn finish(mut self) -> Vec<CodeAction> {
        for action in &mut self.buf {
            let Some(repair) = action.id.repair else {
                continue;
            };
            let matched: Vec<Diagnostic> =
                self.diagnostics.iter().filter(|diag| repair.matches(diag)).cloned().collect();
            if !matched.is_empty() {
                action.id.kind = super::CodeActionKind::QuickFix;
                action.diagnostics = matched;
            }
        }
        self.buf.sort_by_key(|assist| assist.target.len());
        self.buf
    }
}
