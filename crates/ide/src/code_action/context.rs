use hir::semantics::{ParsedFile, Semantics};
use syntax::{
    SyntaxNode,
    ast::{AstNode, CompilationUnit},
};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

use super::{CodeActionDiagnostics, RepairKind};
use crate::db::root_db::RootDb;

pub(crate) struct CodeActionCtx<'a> {
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    range: TextRange,
    diagnostics: CodeActionDiagnostics,
    parsed_file: ParsedFile,
}

impl<'a> CodeActionCtx<'a> {
    pub(super) fn new(
        sema: &'a Semantics<'a, RootDb>,
        file_id: FileId,
        range: TextRange,
        diagnostics: CodeActionDiagnostics,
    ) -> Option<Self> {
        let parsed_file = sema.parse_file(file_id);
        parsed_file.compilation_unit()?;
        Some(Self { sema, file_id, range, diagnostics, parsed_file })
    }

    pub(crate) fn sema(&self) -> &'a Semantics<'a, RootDb> {
        self.sema
    }

    pub(crate) fn file_id(&self) -> FileId {
        self.file_id
    }

    pub(crate) fn range(&self) -> TextRange {
        self.range
    }

    pub(crate) fn syntax(&self) -> SyntaxNode<'_> {
        self.compilation_unit().syntax()
    }

    pub(crate) fn allows_repair(&self, repair: RepairKind) -> bool {
        self.diagnostics.allows_repair(repair)
    }

    fn offset(&self) -> TextSize {
        self.range.start()
    }

    pub(crate) fn find_node_at_offset<'b, N: AstNode<'b>>(&'b self) -> Option<N> {
        self.sema.find_node_at_offset(self.compilation_unit().syntax(), self.offset())
    }

    fn compilation_unit(&self) -> CompilationUnit<'_> {
        self.parsed_file
            .compilation_unit()
            .expect("CodeActionCtx should only be constructed for compilation units")
    }
}
