use std::ops::Range;

use base_db::{Cancelled, salsa, source_db::SourceDb};
use ide_db::{line_index_db::LineIndexDb, root_db::RootDb};
use span::{FilePosition, RangeInfo};
use triomphe::Arc;
use utils::{
    line_index::{LineIndex, TextRange},
    lines::LineInfo,
    text_edit::TextEdit,
};
use vfs::FileId;

use crate::{
    Cancellable,
    code_action::{self, CodeAction, CodeActionResolveStrategy},
    code_lens::{self, CodeLens, CodeLensConfig, CodeLensKind},
    completion::{
        CompletionItem,
        context::{CompletionContext, TriggerChar},
    },
    diagnostics,
    document_highlight::{self, DocumentHighlight, DocumentHighlightConfig},
    document_symbols::{self, DocumentSymbol},
    folding_ranges::{self, Fold, FoldingConfig},
    formatting::{self, FmtConfig},
    goto_declaration, goto_definition,
    hover::{self, HoverConfig},
    inlay_hint::{self, InlayHint, InlayHintConfig},
    markup::Markup,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
    rename::{self, RenameConfig, RenameResult},
    selection_ranges,
    semantic_tokens::{self, SemaToken, SemaTokenConfig},
    signature_help::{self, SignatureHelp, SignatureHelpConfig},
    source_change::SourceChange,
};

#[derive(Debug)]
pub struct Analysis {
    pub(crate) db: salsa::Snapshot<RootDb>,
}

impl Analysis {
    fn with_db<F, T>(&self, f: F) -> Cancellable<T>
    where
        F: FnOnce(&RootDb) -> T + std::panic::UnwindSafe,
    {
        Cancelled::catch(|| f(&self.db))
    }

    pub fn line_index(&self, file_id: FileId) -> Cancellable<Arc<LineIndex>> {
        self.with_db(|db| db.line_index(file_id))
    }

    pub fn file_text(&self, file_id: FileId) -> Cancellable<Arc<str>> {
        self.with_db(|db| db.file_text(file_id))
    }

    pub fn diagnostics(&self, file_id: FileId) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::parse_diagnostics(db, file_id))
    }
}

impl Analysis {
    pub fn goto_definition(
        &self,
        position: FilePosition,
    ) -> Cancellable<Option<RangeInfo<Vec<NavTarget>>>> {
        self.with_db(|db| goto_definition::goto_definition(db, position))
    }

    pub fn goto_declaration(
        &self,
        position: FilePosition,
    ) -> Cancellable<Option<RangeInfo<Vec<NavTarget>>>> {
        self.with_db(|db| goto_declaration::goto_declaration(db, position))
    }

    pub fn document_symbol(&self, file_id: FileId) -> Cancellable<Vec<DocumentSymbol>> {
        self.with_db(|db| document_symbols::document_symbols(db, file_id))
    }

    pub fn document_highlight(
        &self,
        position: FilePosition,
        config: DocumentHighlightConfig,
    ) -> Cancellable<Option<Vec<DocumentHighlight>>> {
        self.with_db(|db| document_highlight::document_highlight(db, position, config))
    }

    pub fn references(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Cancellable<Option<Vec<References>>> {
        self.with_db(|db| references::references(db, position, config))
    }

    pub fn prepare_rename(&self, position: FilePosition) -> Cancellable<RenameResult<TextRange>> {
        self.with_db(|db| rename::prepare_rename(db, position))
    }

    pub fn rename(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
    ) -> Cancellable<RenameResult<SourceChange>> {
        self.with_db(|db| rename::rename(db, position, config, new_name))
    }

    pub fn format(
        &self,
        file_id: FileId,
        line_range: Option<Range<usize>>,
        line_info: &LineInfo,
        config: FmtConfig,
    ) -> Cancellable<anyhow::Result<Option<TextEdit>>> {
        self.with_db(|db| formatting::format(db, file_id, line_range, line_info, config))
    }

    pub fn format_on_type(
        &self,
        position: FilePosition,
        ch: String,
        line_info: &LineInfo,
        config: FmtConfig,
    ) -> Cancellable<anyhow::Result<Option<TextEdit>>> {
        self.with_db(|db| formatting::format_on_type(db, position, ch, line_info, config))
    }

    pub fn selection_ranges(&self, position: FilePosition) -> Cancellable<Vec<TextRange>> {
        self.with_db(|db| selection_ranges::selection_ranges(db, position))
    }

    pub fn folding_ranges(
        &self,
        file_id: FileId,
        config: &FoldingConfig,
    ) -> Cancellable<Vec<Fold>> {
        self.with_db(|db| folding_ranges::folding_ranges(db, file_id, config))
    }

    pub fn hover(
        &self,
        position: FilePosition,
        config: HoverConfig,
    ) -> Cancellable<Option<RangeInfo<Markup>>> {
        self.with_db(|db| hover::hover(db, position, config))
    }

    pub fn inlay_hint(
        &self,
        file_id: FileId,
        range: TextRange,
        config: InlayHintConfig,
    ) -> Cancellable<Vec<InlayHint>> {
        self.with_db(|db| inlay_hint::inlay_hint(db, file_id, range, config))
    }

    pub fn code_lens(&self, file_id: FileId, config: CodeLensConfig) -> Cancellable<Vec<CodeLens>> {
        self.with_db(|db| code_lens::code_lens(db, config, file_id))
    }

    pub fn code_lens_resolve(&self, kind: CodeLensKind) -> Cancellable<CodeLensKind> {
        self.with_db(|db| code_lens::code_lens_resolve(db, kind))
    }

    pub fn semantic_tokens(
        &self,
        file_id: FileId,
        config: SemaTokenConfig,
        range: Option<TextRange>,
    ) -> Cancellable<Vec<SemaToken>> {
        self.with_db(|db| semantic_tokens::semantic_tokens(db, config, file_id, range))
    }

    pub fn signature_help(
        &self,
        position: FilePosition,
        config: SignatureHelpConfig,
    ) -> Cancellable<Option<SignatureHelp>> {
        self.with_db(|db| signature_help::signature_help(db, position, config))
    }

    pub fn completion_context(&self, position: FilePosition) -> Cancellable<CompletionContext> {
        self.completion_context_with_trigger(position, None)
    }

    pub fn completion_context_with_trigger(
        &self,
        position: FilePosition,
        trigger: Option<TriggerChar>,
    ) -> Cancellable<CompletionContext> {
        self.with_db(|db| crate::completion::context::completion_context(db, position, trigger))
    }

    pub fn completions_with_trigger(
        &self,
        position: FilePosition,
        trigger: Option<TriggerChar>,
    ) -> Cancellable<Vec<CompletionItem>> {
        self.with_db(|db| crate::completion::completions(db, position, trigger))
    }

    pub fn code_action(
        &self,
        file_id: FileId,
        range: TextRange,
        resolve_strategy: CodeActionResolveStrategy,
    ) -> Cancellable<Vec<CodeAction>> {
        self.with_db(|db| code_action::code_action(db, file_id, range, resolve_strategy))
    }
}
