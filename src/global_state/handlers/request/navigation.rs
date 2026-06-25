use ide::{
    FileRange, SymbolKind, navigation_target::NavTarget, references::References,
    semantic_index::ModuleCallItem,
};
use itertools::Itertools;

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
    for edge in snap.analysis.module_incoming_calls(target_file_id, target_selection_range)? {
        let caller = call_hierarchy_item_for_module(&snap, &edge.caller)?;
        let line_info = snap.line_info(edge.caller.file_id)?;
        push_call_range(&mut groups, caller, to_proto::range(&line_info, edge.call_range));
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
    let caller_selection_range = from_proto::text_range(&caller_line_info, caller.selection_range)?;

    let mut groups = Vec::<(lsp_types::CallHierarchyItem, Vec<lsp_types::Range>)>::new();
    for edge in snap.analysis.module_outgoing_calls(caller_file_id, caller_selection_range)? {
        let callee = call_hierarchy_item_for_module(&snap, &edge.callee)?;
        if same_call_hierarchy_item(&caller, &callee) {
            continue;
        }

        push_call_range(&mut groups, callee, to_proto::range(&caller_line_info, edge.call_range));
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

fn call_hierarchy_item_for_module(
    snap: &GlobalStateSnapshot,
    module: &ModuleCallItem,
) -> anyhow::Result<lsp_types::CallHierarchyItem> {
    let uri = to_proto::url(snap, module.file_id)?;
    let line_info = snap.line_info(module.file_id)?;
    Ok(lsp_types::CallHierarchyItem {
        name: module.name.clone(),
        kind: to_proto::symbol_kind(SymbolKind::Module),
        tags: None,
        detail: None,
        uri,
        range: to_proto::range(&line_info, module.full_range),
        selection_range: to_proto::range(&line_info, module.name_range),
        data: None,
    })
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
