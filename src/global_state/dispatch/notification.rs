use lsp_server::Notification;

use crate::global_state::{GlobalState, dispatcher::NotifDispatcher, handlers};

impl GlobalState {
    pub(in crate::global_state) fn handle_notification(&mut self, notif: Notification) {
        use handlers::notification::*;
        use lsp_types::notification::*;

        let mut dispatcher = NotifDispatcher { notif: Some(notif), global_state: self };
        dispatcher
            .on_sync_mut::<Cancel>(handle_cancel)
            .on_sync_mut::<WorkDoneProgressCancel>(handle_work_done_progress_cancel)
            .on_sync_mut::<DidOpenTextDocument>(handle_did_open_text_document)
            .on_sync_mut::<DidChangeTextDocument>(handle_did_change_text_document)
            .on_sync_mut::<DidCloseTextDocument>(handle_did_close_text_document)
            .on_sync_mut::<DidSaveTextDocument>(handle_did_save_text_document)
            .on_sync_mut::<DidChangeConfiguration>(handle_did_change_configuration)
            .on_sync_mut::<DidChangeWorkspaceFolders>(handle_did_change_workspace_folders)
            .on_sync_mut::<DidChangeWatchedFiles>(handle_did_change_watched_files)
            .on_sync_mut::<SetTrace>(handle_set_trace)
            .finish();
    }
}
