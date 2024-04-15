use crate::{file::{HirFileId, InFile}, hir_def::Ident};
use syntax::ast::{self, AstNode};

pub(crate) trait Lower {
    fn file_id(&self) -> HirFileId;

    fn in_file<T>(&self, value: T) -> InFile<T> {
        InFile::new(self.file_id(), value)
    }

    fn file_text(&self) -> &str;

    fn lower_ident(&self, ident: &ast::Identifier) -> Option<Ident> {
        Some(ident.to_text(self.file_text())?.into())
    }
}
