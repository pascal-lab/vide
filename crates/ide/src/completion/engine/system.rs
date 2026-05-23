use std::sync::OnceLock;

use syntax::Compilation as SlangCompilation;

use super::candidate::CompletionCandidate;
use crate::completion::context::CompletionContext;

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

fn system_function_names() -> &'static [String] {
    static SYSTEM_FUNCTION_NAMES: OnceLock<Vec<String>> = OnceLock::new();
    SYSTEM_FUNCTION_NAMES.get_or_init(SlangCompilation::system_function_names)
}

fn system_task_names() -> &'static [String] {
    static SYSTEM_TASK_NAMES: OnceLock<Vec<String>> = OnceLock::new();
    SYSTEM_TASK_NAMES.get_or_init(SlangCompilation::system_task_names)
}
