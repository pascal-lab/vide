use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use syntax::ast::{AstNode, CompilationUnit};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

use crate::source_change::{SourceChange, SourceChangeBuilder};

#[derive(Debug, Clone)]
pub enum CodeActionResolveStrategy {
    None,
    All,
    Single { name: String },
}

impl CodeActionResolveStrategy {
    pub fn is_none(&self) -> bool {
        matches!(self, CodeActionResolveStrategy::None)
    }

    pub fn should_resolve(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::All => true,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }

    pub fn should_add(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::All | CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeActionId {
    pub name: &'static str,
    pub kind: CodeActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Generate,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
}

impl CodeActionKind {
    pub fn contains(self, other: CodeActionKind) -> bool {
        if self == other {
            return true;
        }

        match self {
            CodeActionKind::Generate => true,
            CodeActionKind::Refactor => matches!(
                other,
                CodeActionKind::RefactorExtract
                    | CodeActionKind::RefactorInline
                    | CodeActionKind::RefactorRewrite
            ),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeAction {
    pub id: CodeActionId,
    pub label: String,
    /// Target ranges are used to sort assists: the smaller the target range,
    /// the more specific assist is, and so it should be sorted first.
    pub target: TextRange,
    /// Compute it lazily
    pub source_change: Option<SourceChange>,
}

pub(crate) struct CodeActionCollector {
    file: FileId,
    resolve_strategy: CodeActionResolveStrategy,
    buf: Vec<CodeAction>,
}

impl CodeActionCollector {
    fn new(ctx: &CodeActionCtx, resolve_strategy: CodeActionResolveStrategy) -> Self {
        Self { file: ctx.file_id, resolve_strategy, buf: Vec::new() }
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

    fn finish(mut self) -> Vec<CodeAction> {
        self.buf.sort_by_key(|assist| assist.target.len());
        self.buf
    }
}

struct CodeActionCtx<'a> {
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    range: TextRange,
    compilation_unit: CompilationUnit<'a>,
}

impl<'a> CodeActionCtx<'a> {
    fn new(sema: &'a Semantics<'a, RootDb>, file_id: FileId, range: TextRange) -> Self {
        let compilation_unit = sema.parse(file_id);
        Self { sema, file_id, range, compilation_unit }
    }

    fn offset(&self) -> TextSize {
        self.range.start()
    }

    fn find_node_at_offset<N: AstNode<'a>>(&self) -> Option<N> {
        self.sema.find_node_at_offset(self.compilation_unit.syntax(), self.offset())
    }
}

pub(crate) fn code_action(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    resolve_strategy: CodeActionResolveStrategy,
) -> Vec<CodeAction> {
    let sema = Semantics::new(db);
    let ctx = CodeActionCtx::new(&sema, file_id, range);

    let mut collector = CodeActionCollector::new(&ctx, resolve_strategy);
    handlers::all().iter().for_each(|handler| {
        handler(&mut collector, &ctx);
    });

    collector.finish()
}

mod handlers {
    use super::{CodeActionCollector, CodeActionCtx};

    pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

    mod add_missing_connections;
    mod add_missing_parameters;

    pub(crate) fn all() -> &'static [Handler] {
        &[
            add_missing_connections::add_missing_connections,
            add_missing_parameters::add_missing_parameters,
        ]
    }
}
