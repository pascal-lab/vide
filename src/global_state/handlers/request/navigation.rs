use ide::{
    FilePosition, FileRange, SymbolKind, document_symbols::DocumentSymbol,
    navigation_target::NavTarget, references::References,
};
use itertools::Itertools;
use utils::text_edit::{TextRange, TextSize};

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_goto_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let Some(nav_info) = snap.analysis.goto_definition(position)? else {
        return Ok(None);
    };

    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

pub(crate) fn handle_goto_declaration(
    snap: GlobalStateSnapshot,
    params: lsp_types::request::GotoDeclarationParams,
) -> anyhow::Result<Option<lsp_types::request::GotoDeclarationResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params.clone())?;
    let Some(nav_info) = snap.analysis.goto_declaration(position)? else {
        return handle_goto_definition(snap, params);
    };
    let src = FileRange { file_id: position.file_id, range: nav_info.range };
    let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    Ok(Some(res))
}

pub(crate) fn handle_goto_type_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::request::GotoTypeDefinitionParams,
) -> anyhow::Result<Option<lsp_types::request::GotoTypeDefinitionResponse>> {
    handle_goto_definition(snap, params)
}

pub(crate) fn handle_prepare_call_hierarchy(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyPrepareParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyItem>>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let Some(nav_info) = snap.analysis.goto_definition(position)? else {
        return Ok(None);
    };

    let items = nav_info
        .info
        .into_iter()
        .filter_map(|nav| call_hierarchy_item_for_nav(&snap, nav).transpose())
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .unique_by(call_hierarchy_item_key)
        .collect_vec();

    Ok((!items.is_empty()).then_some(items))
}

pub(crate) fn handle_call_hierarchy_incoming(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyIncomingCallsParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyIncomingCall>>> {
    let target = params.item;
    let target_file_id = from_proto::file_id(&snap, &target.uri)?;
    let target_line_info = snap.line_info(target_file_id)?;
    let target_selection_range = from_proto::text_range(&target_line_info, target.selection_range)?;

    let mut groups = Vec::<(lsp_types::CallHierarchyItem, Vec<lsp_types::Range>)>::new();
    for reference in reference_ranges_for_call_item(&snap, &target)? {
        if reference.file_id == target_file_id && reference.range == target_selection_range {
            continue;
        }

        let Some(caller) = enclosing_module_item(&snap, reference)? else {
            continue;
        };
        let line_info = snap.line_info(reference.file_id)?;
        push_call_range(&mut groups, caller, to_proto::range(&line_info, reference.range));
    }

    let calls = groups
        .into_iter()
        .map(|(from, from_ranges)| lsp_types::CallHierarchyIncomingCall { from, from_ranges })
        .collect_vec();
    Ok((!calls.is_empty()).then_some(calls))
}

pub(crate) fn handle_call_hierarchy_outgoing(
    snap: GlobalStateSnapshot,
    params: lsp_types::CallHierarchyOutgoingCallsParams,
) -> anyhow::Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>> {
    let caller = params.item;
    let caller_file_id = from_proto::file_id(&snap, &caller.uri)?;
    let caller_line_info = snap.line_info(caller_file_id)?;
    let caller_range = from_proto::text_range(&caller_line_info, caller.range)?;

    let mut groups = Vec::<(lsp_types::CallHierarchyItem, Vec<lsp_types::Range>)>::new();
    for callee in workspace_module_items(&snap)? {
        if same_call_hierarchy_item(&caller, &callee) {
            continue;
        }

        for reference in reference_ranges_for_call_item(&snap, &callee)? {
            if reference.file_id == caller_file_id
                && range_contains_range(caller_range, reference.range)
            {
                push_call_range(
                    &mut groups,
                    callee.clone(),
                    to_proto::range(&caller_line_info, reference.range),
                );
            }
        }
    }

    let calls = groups
        .into_iter()
        .map(|(to, from_ranges)| lsp_types::CallHierarchyOutgoingCall { to, from_ranges })
        .collect_vec();
    Ok((!calls.is_empty()).then_some(calls))
}

pub(crate) fn handle_document_highlight(
    snap: GlobalStateSnapshot,
    params: lsp_types::DocumentHighlightParams,
) -> anyhow::Result<Option<Vec<lsp_types::DocumentHighlight>>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    let line_info = snap.line_info(position.file_id)?;
    let config = snap.config.document_highlight();
    let Some(highlights) = snap.analysis.document_highlight(position, config)? else {
        return Ok(None);
    };

    let res = highlights
        .into_iter()
        .map(|highlight| to_proto::document_highlight(&line_info, highlight))
        .collect();
    Ok(Some(res))
}

pub(crate) fn handle_references(
    snap: GlobalStateSnapshot,
    params: lsp_types::ReferenceParams,
) -> anyhow::Result<Option<Vec<lsp_types::Location>>> {
    let include_declaration =
        params.context.include_declaration && snap.config.references_include_declaration();
    let position = from_proto::file_position(&snap, params.text_document_position)?;
    let config = snap.config.references();
    let Some(refs) = snap.analysis.references(position, config)? else {
        return Ok(None);
    };
    let partial_issue_count: usize =
        refs.iter().map(|references| references.status.issue_count()).sum();
    if partial_issue_count > 0 {
        tracing::debug!(partial_issue_count, "references result is partial");
    }

    let locations = refs
        .into_iter()
        .flat_map(|References { def, refs, .. }| {
            let decl = if include_declaration { def.unwrap_or_default() } else { Vec::new() }
                .into_iter()
                .map(|nav| FileRange { file_id: nav.file_id, range: nav.focus_or_full_range() });

            let refs = refs.into_iter().flat_map(|(file_id, refs)| {
                refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
            });

            decl.chain(refs)
        })
        .unique()
        .filter_map(|frange| to_proto::location(&snap, frange).ok())
        .collect_vec();

    Ok(Some(locations))
}

fn reference_ranges_for_call_item(
    snap: &GlobalStateSnapshot,
    item: &lsp_types::CallHierarchyItem,
) -> anyhow::Result<Vec<FileRange>> {
    let file_id = from_proto::file_id(snap, &item.uri)?;
    let line_info = snap.line_info(file_id)?;
    let offset = from_proto::offset(&line_info, item.selection_range.start)?;
    let position = FilePosition { file_id, offset };
    let config = snap.config.references();
    let Some(references) = snap.analysis.references(position, config)? else {
        return Ok(Vec::new());
    };

    Ok(references
        .into_iter()
        .flat_map(|References { refs, .. }| {
            refs.into_iter().flat_map(|(file_id, refs)| {
                refs.into_iter().map(move |(range, _)| FileRange { file_id, range })
            })
        })
        .unique()
        .collect_vec())
}

fn call_hierarchy_item_for_nav(
    snap: &GlobalStateSnapshot,
    nav: NavTarget,
) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
    let Some(kind) = nav.kind else {
        return Ok(None);
    };
    if !is_call_hierarchy_kind(kind) {
        return Ok(None);
    }

    let line_info = snap.line_info(nav.file_id)?;
    let uri = to_proto::url(snap, nav.file_id)?;
    let range = to_proto::range(&line_info, nav.full_range);
    let selection_range = to_proto::range(&line_info, nav.focus_or_full_range());
    let name = nav
        .name
        .map(|name| name.to_string())
        .unwrap_or_else(|| nav.description.clone().unwrap_or_else(|| "<anonymous>".to_owned()));
    let detail = nav.container_name.map(|name| name.to_string()).or(nav.description);

    Ok(Some(lsp_types::CallHierarchyItem {
        name,
        kind: to_proto::symbol_kind(kind),
        tags: None,
        detail,
        uri,
        range,
        selection_range,
        data: None,
    }))
}

fn workspace_module_items(
    snap: &GlobalStateSnapshot,
) -> anyhow::Result<Vec<lsp_types::CallHierarchyItem>> {
    let mut file_ids = snap.file_ids();
    file_ids.sort_unstable_by_key(|file_id| file_id.0);
    file_ids.dedup();

    let mut items = Vec::new();
    for file_id in file_ids {
        let uri = to_proto::url(snap, file_id)?;
        let line_info = snap.line_info(file_id)?;
        for symbol in snap.analysis.document_symbol(file_id)? {
            collect_module_items(&uri, &line_info, symbol, &mut items);
        }
    }

    Ok(items.into_iter().unique_by(call_hierarchy_item_key).collect())
}

fn collect_module_items(
    uri: &lsp_types::Url,
    line_info: &utils::lines::LineInfo,
    symbol: DocumentSymbol,
    items: &mut Vec<lsp_types::CallHierarchyItem>,
) {
    if symbol.kind == SymbolKind::Module {
        items.push(call_hierarchy_item_for_symbol(uri, line_info, &symbol));
    }

    for child in symbol.children {
        collect_module_items(uri, line_info, child, items);
    }
}

fn enclosing_module_item(
    snap: &GlobalStateSnapshot,
    range: FileRange,
) -> anyhow::Result<Option<lsp_types::CallHierarchyItem>> {
    let uri = to_proto::url(snap, range.file_id)?;
    let line_info = snap.line_info(range.file_id)?;
    let mut best = None;
    for symbol in snap.analysis.document_symbol(range.file_id)? {
        find_enclosing_module_symbol(symbol, range.range.start(), &mut best);
    }

    Ok(best.map(|symbol| call_hierarchy_item_for_symbol(&uri, &line_info, &symbol)))
}

fn find_enclosing_module_symbol(
    symbol: DocumentSymbol,
    offset: TextSize,
    best: &mut Option<DocumentSymbol>,
) {
    if !range_contains_offset(symbol.full_range, offset) {
        return;
    }

    if symbol.kind == SymbolKind::Module
        && best.as_ref().is_none_or(|current| symbol.full_range.len() < current.full_range.len())
    {
        *best = Some(symbol.clone());
    }

    for child in symbol.children {
        find_enclosing_module_symbol(child, offset, best);
    }
}

fn call_hierarchy_item_for_symbol(
    uri: &lsp_types::Url,
    line_info: &utils::lines::LineInfo,
    symbol: &DocumentSymbol,
) -> lsp_types::CallHierarchyItem {
    lsp_types::CallHierarchyItem {
        name: symbol.name.clone(),
        kind: to_proto::symbol_kind(symbol.kind),
        tags: None,
        detail: symbol.container_name.clone(),
        uri: uri.clone(),
        range: to_proto::range(line_info, symbol.full_range),
        selection_range: to_proto::range(line_info, symbol.focus_range),
        data: None,
    }
}

fn is_call_hierarchy_kind(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Module)
}

fn push_call_range(
    groups: &mut Vec<(lsp_types::CallHierarchyItem, Vec<lsp_types::Range>)>,
    item: lsp_types::CallHierarchyItem,
    range: lsp_types::Range,
) {
    if let Some((_, ranges)) =
        groups.iter_mut().find(|(existing, _)| same_call_hierarchy_item(existing, &item))
    {
        if !ranges.contains(&range) {
            ranges.push(range);
        }
        return;
    }

    groups.push((item, vec![range]));
}

fn same_call_hierarchy_item(
    lhs: &lsp_types::CallHierarchyItem,
    rhs: &lsp_types::CallHierarchyItem,
) -> bool {
    lhs.name == rhs.name
        && lhs.uri == rhs.uri
        && lhs.range == rhs.range
        && lhs.selection_range == rhs.selection_range
}

fn call_hierarchy_item_key(
    item: &lsp_types::CallHierarchyItem,
) -> (String, lsp_types::Url, lsp_types::Range, lsp_types::Range) {
    (item.name.clone(), item.uri.clone(), item.range, item.selection_range)
}

fn range_contains_offset(range: TextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset < range.end()
}

fn range_contains_range(container: TextRange, range: TextRange) -> bool {
    container.start() <= range.start() && range.end() <= container.end()
}

pub(crate) fn handle_hover(
    snap: GlobalStateSnapshot,
    params: lsp_types::HoverParams,
) -> anyhow::Result<Option<lsp_types::Hover>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;

    let config = snap.config.hover();
    let hover_format = config.format;
    let Some(hover_info) = snap.analysis.hover(position, config)? else {
        return Ok(None);
    };

    let line_info = snap.line_info(position.file_id)?;
    let range = to_proto::range(&line_info, hover_info.range);

    let res = lsp_types::Hover {
        contents: to_proto::hover_contents(hover_info.info, hover_format),
        range: Some(range),
    };

    Ok(Some(res))
}
