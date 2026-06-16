mod lifecycle;
mod text_changes;
mod text_document;
mod workspace;

pub(crate) use lifecycle::{handle_cancel, handle_set_trace, handle_work_done_progress_cancel};
pub(crate) use text_document::{
    handle_did_change_text_document, handle_did_close_text_document, handle_did_open_text_document,
    handle_did_save_text_document,
};
pub(crate) use workspace::{
    handle_did_change_configuration, handle_did_change_watched_files,
    handle_did_change_workspace_folders,
};
