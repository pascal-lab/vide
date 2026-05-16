use hir::{db::HirDb, file::HirFileId, hir_def::module::ModuleId, source_map::IsSrc};
use ide_db::root_db::RootDb;
use span::FilePosition;
use utils::{
    get::Get,
    text_edit::{TextEditItem, TextSize},
};

use super::{CompletionItem, CompletionItemKind, typed_filter::value_candidates_in_module};
use crate::completion::{
    context::{CompletionContext, ExpectedSyntax},
    syntax_keywords,
};

pub(super) fn complete_sensitivity_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let wrap_in_parens = matches!(
        ctx.expectation.map(|expectation| expectation.syntax),
        Some(ExpectedSyntax::EventControl { wrap_in_parens: true })
    );
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
    items: &mut Vec<CompletionItem>,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
    prefix: &str,
) {
    let label = "*";
    if !label.starts_with(prefix) {
        return;
    }

    let plain = if wrap_in_parens { "(*)".to_string() } else { "*".to_string() };
    items.push(CompletionItem {
        label: label.to_string(),
        kind: CompletionItemKind::Snippet,
        edit: Some(TextEditItem::replace(ctx.replacement, plain.clone())),
        snippet_edit: Some(TextEditItem::replace(ctx.replacement, plain)),
    });
}

fn push_event_keywords(
    items: &mut Vec<CompletionItem>,
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

        items.push(CompletionItem {
            label: keyword.to_string(),
            kind: CompletionItemKind::Snippet,
            edit: Some(TextEditItem::replace(ctx.replacement, plain)),
            snippet_edit: Some(TextEditItem::replace(ctx.replacement, snippet)),
        });
    }
}

fn signal_candidates(
    db: &RootDb,
    module_id: ModuleId,
    prefix: &str,
    ctx: &CompletionContext,
    wrap_in_parens: bool,
) -> Vec<CompletionItem> {
    value_candidates_in_module(db, module_id)
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .map(|(name, _)| {
            let plain = if wrap_in_parens { format!("({name})") } else { name.clone() };
            CompletionItem {
                label: name.clone(),
                kind: CompletionItemKind::Text,
                edit: Some(TextEditItem::replace(ctx.replacement, plain)),
                snippet_edit: None,
            }
        })
        .collect()
}
