use crate::global_state::GlobalState;

pub(crate) fn handle_cancel(
    state: &mut GlobalState,
    params: lsp_types::CancelParams,
) -> anyhow::Result<()> {
    let id: lsp_server::RequestId = match params.id {
        lsp_types::NumberOrString::Number(id) => id.into(),
        lsp_types::NumberOrString::String(id) => id.into(),
    };
    state.cancel(id);
    Ok(())
}

pub(crate) fn handle_work_done_progress_cancel(
    state: &mut GlobalState,
    params: lsp_types::WorkDoneProgressCancelParams,
) -> anyhow::Result<()> {
    state.cancel_work_done_progress(params);
    Ok(())
}

pub(crate) fn handle_set_trace(
    state: &mut GlobalState,
    params: lsp_types::SetTraceParams,
) -> anyhow::Result<()> {
    state.set_lsp_trace(params.value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lsp_server::Connection;
    use lsp_types::{SetTraceParams, TraceValue};
    use utils::paths::AbsPathBuf;

    use super::handle_set_trace;
    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    fn test_state() -> (GlobalState, Connection) {
        let root_path = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        let config = config::Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );

        let (server, client) = Connection::memory();
        (GlobalState::new(server.sender, config, TraceValue::Off), client)
    }

    #[test]
    fn set_trace_notification_updates_server_trace_level() {
        let (mut state, client) = test_state();

        handle_set_trace(&mut state, SetTraceParams { value: TraceValue::Verbose }).unwrap();

        assert_eq!(state.lsp_trace.level(), TraceValue::Verbose);
        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
    }
}
