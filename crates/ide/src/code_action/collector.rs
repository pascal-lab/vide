use utils::text_edit::TextRange;
use vfs::FileId;

use super::{CodeAction, CodeActionId, CodeActionResolveStrategy};
use crate::source_change::SourceChangeBuilder;

pub(crate) struct CodeActionCollector {
    file: FileId,
    resolve_strategy: CodeActionResolveStrategy,
    buf: Vec<CodeAction>,
}

impl CodeActionCollector {
    pub(super) fn new(file: FileId, resolve_strategy: CodeActionResolveStrategy) -> Self {
        Self { file, resolve_strategy, buf: Vec::new() }
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

        self.buf.push(CodeAction { id, label: label.into(), target, source_change });
        Some(())
    }

    pub(super) fn finish(mut self) -> Vec<CodeAction> {
        self.buf.sort_by_key(|assist| assist.target.len());
        self.buf
    }
}
