use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use syntax::{
    SyntaxNode,
    ast::{AstNode, CompilationUnit},
};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

use super::{CodeActionDiagnostics, RepairKind};

pub(crate) struct CodeActionCtx<'a> {
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    range: TextRange,
    diagnostics: CodeActionDiagnostics,
    compilation_unit: CompilationUnit<'a>,
}

impl<'a> CodeActionCtx<'a> {
    pub(super) fn new(
        sema: &'a Semantics<'a, RootDb>,
        file_id: FileId,
        range: TextRange,
        diagnostics: CodeActionDiagnostics,
    ) -> Option<Self> {
        let compilation_unit = CompilationUnit::cast(sema.parse_root(file_id)?)?;
        Some(Self { sema, file_id, range, diagnostics, compilation_unit })
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

    pub(crate) fn syntax(&self) -> SyntaxNode<'a> {
        self.compilation_unit.syntax()
    }

    pub(crate) fn allows_repair(&self, repair: RepairKind) -> bool {
        self.diagnostics.allows_repair(repair)
    }

    fn offset(&self) -> TextSize {
        self.range.start()
    }

    pub(crate) fn find_node_at_offset<N: AstNode<'a>>(&self) -> Option<N> {
        self.sema.find_node_at_offset(self.compilation_unit.syntax(), self.offset())
    }
}
