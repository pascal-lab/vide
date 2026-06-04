use hir::base_db::source_db::SourceRootDb;

use super::candidate::CompletionCandidate;
use crate::{completion::context::CompletionContext, db::root_db::RootDb};

pub(super) fn complete_system_tasks(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let names = db.system_task_names();
    collect_system_subroutines(prefix, ctx, &names)
}

pub(super) fn complete_system_functions(
    db: &RootDb,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let names = db.system_function_names();
    collect_system_subroutines(prefix, ctx, &names)
}

fn collect_system_subroutines(
    prefix: &str,
    ctx: &CompletionContext,
    names: &[String],
) -> Vec<CompletionCandidate> {
    if !prefix.starts_with('$') {
        return Vec::new();
    }

    names
        .iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| {
            let snippet_name = name.replacen('$', r"\$", 1);
            CompletionCandidate::semantic_snippet(
                name.clone(),
                ctx.replacement,
                format!("{name}()"),
                format!("{snippet_name}(${{1:args}})"),
            )
        })
        .collect()
}
