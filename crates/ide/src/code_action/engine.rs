
use utils::text_edit::TextRange;
use vfs::FileId;

use super::{CodeAction, CodeActionCollector, CodeActionCtx, CodeActionResolveStrategy, handlers};
use crate::{db::root_db::RootDb, diagnostics::Diagnostic};

pub(crate) fn code_action(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    diagnostics: &[Diagnostic],
    resolve_strategy: CodeActionResolveStrategy,
) -> Vec<CodeAction> {
    if db.file_kind(file_id).is_project_manifest() {
        return Vec::new();
    }
    let sema = db.semantics();
    let Some(ctx) = CodeActionCtx::new(&sema, file_id, range, diagnostics) else {
        return Vec::new();
    };

    let mut collector = CodeActionCollector::new(ctx.file_id(), resolve_strategy, diagnostics);
    handlers::all().iter().for_each(|handler| {
        handler(&mut collector, &ctx);
    });

    collector.finish()
}
