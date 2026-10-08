use hir_def::{has_source::HasSource, owner::OwnerId};
use preproc_expand::file::HirFileId;
use utils::text_edit::TextSize;

use super::{candidate::CompletionCandidate, typed_filter::value_candidates_in_module};
use crate::{
    FilePosition,
    analysis::AnalysisContext,
    completion::{context::CompletionContext, syntax_keywords},
};

pub(super) fn complete_sensitivity_list(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
) -> Vec<CompletionCandidate> {
    let mut items = Vec::new();

    push_star_item(&mut items, ctx, wrap_in_parens, prefix);
    push_event_keywords(&mut items, ctx, wrap_in_parens, prefix);

    if let Some(module_id) = module_id_at_offset(db, position) {
        items.extend(signal_candidates(db, module_id, prefix, ctx, wrap_in_parens));
    }

    items
}

fn module_id_at_offset(db: &AnalysisContext<'_>, position: FilePosition) -> Option<OwnerId> {
    let file_id = HirFileId::File(position.file_id);
    let hir_file =
        db.body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"));
    let mut best: Option<(TextSize, OwnerId)> = None;

    for module_id in hir_file.module_owners() {
        let Some(range) = module_id.source(db.db).map(|source| source.value.full_range()) else {
            continue;
        };
        if !range.contains(position.offset) && range.end() != position.offset {
            continue;
        }

        let len = range.len();
        match best {
            None => best = Some((len, module_id)),
            Some((best_len, _)) if len < best_len => best = Some((len, module_id)),
            _ => {}
        }
    }

    best.map(|(_, module_id)| module_id)
}

fn push_star_item(
    items: &mut Vec<CompletionCandidate>,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
    prefix: &str,
) {
    let label = "*";
    if !label.starts_with(prefix) {
        return;
    }

    let plain = if wrap_in_parens { "(*)".to_string() } else { "*".to_string() };
    items.push(CompletionCandidate::snippet(label, ctx.replacement, plain.clone(), plain));
}

fn push_event_keywords(
    items: &mut Vec<CompletionCandidate>,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
    prefix: &str,
) {
    for keyword in syntax_keywords::edge_keywords() {
        if !keyword.starts_with(prefix) {
            continue;
        }

        let (plain, snippet) = if wrap_in_parens {
            (format!("({keyword} )"), format!("({keyword} ${{1:signal}})"))
        } else {
            (format!("{keyword} "), format!("{keyword} ${{1:signal}}"))
        };

        items.push(CompletionCandidate::snippet(keyword.clone(), ctx.replacement, plain, snippet));
    }
}

fn signal_candidates(
    db: &AnalysisContext<'_>,
    module_id: OwnerId,
    prefix: &str,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
) -> Vec<CompletionCandidate> {
    value_candidates_in_module(db, module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| {
            let plain = if wrap_in_parens { format!("({name})") } else { name.clone() };
            CompletionCandidate::text_edit(name, ctx.replacement, plain)
        })
        .collect()
}
