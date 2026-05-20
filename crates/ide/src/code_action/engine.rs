use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use utils::text_edit::TextRange;
use vfs::FileId;

use super::{
    CodeAction, CodeActionCollector, CodeActionCtx, CodeActionDiagnostics,
    CodeActionResolveStrategy, handlers,
};

pub(crate) fn code_action(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    diagnostics: CodeActionDiagnostics,
    resolve_strategy: CodeActionResolveStrategy,
) -> Vec<CodeAction> {
    let sema = Semantics::new(db);
    let Some(ctx) = CodeActionCtx::new(&sema, file_id, range, diagnostics) else {
        return Vec::new();
    };

    let mut collector = CodeActionCollector::new(ctx.file_id(), resolve_strategy);
    handlers::all().iter().for_each(|handler| {
        handler(&mut collector, &ctx);
    });

    collector.finish()
}
