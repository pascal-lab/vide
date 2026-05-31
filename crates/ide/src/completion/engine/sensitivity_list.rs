use hir::{db::HirDb, file::HirFileId, hir_def::module::ModuleId, source_map::IsSrc};
use utils::{get::Get, text_edit::TextSize};

use super::{candidate::CompletionCandidate, typed_filter::value_candidates_in_module};
use crate::{
    FilePosition,
    completion::{context::CompletionContext, syntax_keywords},
    db::root_db::RootDb,
};

pub(super) fn complete_sensitivity_list(
    db: &RootDb,
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

fn module_id_at_offset(db: &RootDb, position: FilePosition) -> Option<ModuleId> {
    let file_id = HirFileId(position.file_id);
    let (hir_file, file_src_map) = db.hir_file_with_source_map(file_id);
    let mut best: Option<(TextSize, ModuleId)> = None;

    for (local_module_id, _) in hir_file.modules.iter() {
        let Some(range) = file_src_map.get(local_module_id).map(|src| src.range()) else {
            continue;
        };
        if !range.contains(position.offset) && range.end() != position.offset {
            continue;
        }

        let len = range.len();
        let module_id = ModuleId::new(file_id, local_module_id);
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
    db: &RootDb,
    module_id: ModuleId,
    prefix: &str,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
) -> Vec<CompletionCandidate> {
    value_candidates_in_module(db, module_id)
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, _)| {
            let plain = if wrap_in_parens { format!("({name})") } else { name.clone() };
            CompletionCandidate::text_edit(name, ctx.replacement, plain)
        })
        .collect()
}
