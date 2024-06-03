use crate::{
    container::{ContainerId, InFile},
    db::InternDb,
    file::HirFileId,
    hir_def::Ident,
};
use syntax::ast::{self, AstNode};

pub(crate) fn lower_ident(ident: &ast::Identifier, file_text: &str) -> Option<Ident> {
    Some(ident.to_text(file_text)?.into())
}

pub(crate) trait Lower {
    type ContainerId: Into<ContainerId>;

    fn db(&self) -> &dyn InternDb;

    fn container_id(&self) -> Self::ContainerId;

    fn file_id(&self) -> HirFileId;

    fn in_file<T>(&self, value: T) -> InFile<T> {
        InFile::new(self.file_id(), value)
    }

    fn file_text(&self) -> &str;

    fn lower_ident(&self, ident: &ast::Identifier) -> Option<Ident> {
        lower_ident(ident, self.file_text())
    }

    fn lower_systf_identifier(&self, ident: &ast::SystemTfIdentifier) -> Option<Ident> {
        ident.to_text(self.file_text()).map(|s| s.into())
    }
}
