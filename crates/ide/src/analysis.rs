use base_db::{Cancelled, salsa};
use ide_db::{line_index_db::LineIndexDb, root_db::RootDb};
use line_index::{LineIndex, TextRange};
use span::{FilePosition, RangeInfo};
use triomphe::Arc;
use utils::{lines::LineInfo, text_edit::TextEdit};
use vfs::FileId;

use crate::{
    Cancellable,
    document_highlight::{self, DocumentHighlight, DocumentHighlightConfig},
    document_symbols::{self, DocumentSymbol},
    formatting::{self, FmtConfig},
    goto_definition,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
    rename::{self, RenameConfig, RenameResult},
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
}

impl Analysis {
    pub fn goto_definition(
        &self,
        position: FilePosition,
    ) -> Cancellable<Option<RangeInfo<Vec<NavTarget>>>> {
        self.with_db(|db| goto_definition::goto_definition(db, position))
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
        line_range: Option<(usize, usize)>,
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
}
