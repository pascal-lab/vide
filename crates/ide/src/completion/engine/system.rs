use super::candidate::CompletionCandidate;
use crate::{
    completion::context::CompletionContext,
    signature_help::{system_function_names, system_task_names},
};

pub(super) fn complete_system_tasks(
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    collect_system_subroutines(prefix, ctx, system_task_names())
}

pub(super) fn complete_system_functions(
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    collect_system_subroutines(prefix, ctx, system_function_names())
}

fn collect_system_subroutines(
    prefix: &str,
    ctx: &CompletionContext,
    names: &[&str],
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
                (*name).to_owned(),
                ctx.replacement,
                format!("{name}()"),
                format!("{snippet_name}(${{1:args}})"),
            )
        })
        .collect()
}
