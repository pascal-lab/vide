use span::FileRange;

use crate::{
    global_state::snapshot::GlobalStateSnapshot,
    lsp_ext::{from_proto, to_proto},
};

pub(crate) fn handle_goto_definition(
    snap: GlobalStateSnapshot,
    params: lsp_types::GotoDefinitionParams,
) -> anyhow::Result<Option<lsp_types::GotoDefinitionResponse>> {
    let position = from_proto::file_position(&snap, params.text_document_position_params)?;
    todo!()
    // let Some(nav_info) = snap.analysis.goto_definition(position)? else {
    //     return Ok(None);
    // };
    // let src = FileRange { file_id: position.file_id, range: nav_info.range };
    // let res = to_proto::goto_definition_response(&snap, Some(src), nav_info.info)?;
    // Ok(Some(res))
}
