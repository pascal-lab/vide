use lsp_server::Message;
use lsp_types::{LogTraceParams, TraceValue, notification, notification::Notification as _};

use crate::global_state::GlobalState;

#[derive(Debug, Clone)]
pub(crate) struct LspTrace {
    level: TraceValue,
}

impl LspTrace {
    pub(crate) fn new(level: TraceValue) -> Self {
        Self { level }
    }

    pub(crate) fn set_level(&mut self, level: TraceValue) -> Option<LogTraceParams> {
        self.level = level;
        self.log_params(format!("trace level set to {}", trace_label(level)), None)
    }

    pub(crate) fn incoming(&self, message: &Message) -> Option<LogTraceParams> {
        self.message_trace("received", message)
    }

    pub(crate) fn outgoing(&self, message: &Message) -> Option<LogTraceParams> {
        self.message_trace("sent", message)
    }

    fn message_trace(&self, direction: &str, message: &Message) -> Option<LogTraceParams> {
        match message {
            Message::Request(request) => self.log_params(
                format!("{direction} request {}", request.method),
                Some(format!("{request:#?}")),
            ),
            Message::Response(response) => self.log_params(
                format!("{direction} response {}", response.id),
                Some(format!("{response:#?}")),
            ),
            Message::Notification(notification)
                if notification.method == notification::LogTrace::METHOD =>
            {
                None
            }
            Message::Notification(notification) => self.log_params(
                format!("{direction} notification {}", notification.method),
                Some(format!("{notification:#?}")),
            ),
        }
    }

    fn log_params(&self, message: String, verbose: Option<String>) -> Option<LogTraceParams> {
        let verbose = match self.level {
            TraceValue::Off => return None,
            TraceValue::Messages => None,
            TraceValue::Verbose => verbose,
        };

        Some(LogTraceParams { message, verbose })
    }
}

impl GlobalState {
    pub(crate) fn set_lsp_trace(&mut self, level: TraceValue) {
        if let Some(params) = self.lsp_trace.set_level(level) {
            self.send_lsp_trace(params);
        }
    }

    pub(crate) fn trace_incoming_lsp_message(&self, message: &Message) {
        if let Some(params) = self.lsp_trace.incoming(message) {
            self.send_lsp_trace(params);
        }
    }

    pub(crate) fn trace_outgoing_lsp_message(&self, message: &Message) {
        if let Some(params) = self.lsp_trace.outgoing(message) {
            self.send_lsp_trace(params);
        }
    }

    fn send_lsp_trace(&self, params: LogTraceParams) {
        let notification =
            lsp_server::Notification::new(notification::LogTrace::METHOD.to_owned(), params);
        self.send_raw(notification.into());
    }
}

fn trace_label(trace: TraceValue) -> &'static str {
    match trace {
        TraceValue::Off => "off",
        TraceValue::Messages => "messages",
        TraceValue::Verbose => "verbose",
    }
}

#[cfg(test)]
mod tests {
    use lsp_server::{Message, Request};
    use lsp_types::{
        TraceValue,
        notification::{LogTrace, Notification as _},
        request::{Request as _, Shutdown},
    };

    use super::LspTrace;

    fn shutdown_request() -> Message {
        Message::Request(Request::new(
            lsp_server::RequestId::from(1),
            Shutdown::METHOD.to_owned(),
            (),
        ))
    }

    #[test]
    fn off_trace_suppresses_messages() {
        let trace = LspTrace::new(TraceValue::Off);

        assert!(trace.incoming(&shutdown_request()).is_none());
    }

    #[test]
    fn messages_trace_omits_verbose_details() {
        let trace = LspTrace::new(TraceValue::Messages);
        let params = trace.incoming(&shutdown_request()).expect("trace params");

        assert_eq!(params.message, "received request shutdown");
        assert!(params.verbose.is_none());
    }

    #[test]
    fn verbose_trace_includes_lsp_message_details() {
        let trace = LspTrace::new(TraceValue::Verbose);
        let params = trace.incoming(&shutdown_request()).expect("trace params");
        let verbose = params.verbose.expect("verbose details");

        assert_eq!(params.message, "received request shutdown");
        assert!(verbose.contains("Request"));
        assert!(verbose.contains("shutdown"));
    }

    #[test]
    fn log_trace_notifications_are_not_traced() {
        let trace = LspTrace::new(TraceValue::Verbose);
        let notification = lsp_server::Notification::new(
            LogTrace::METHOD.to_owned(),
            lsp_types::LogTraceParams { message: "already tracing".to_owned(), verbose: None },
        );

        assert!(trace.outgoing(&Message::Notification(notification)).is_none());
    }

    #[test]
    fn enabling_trace_reports_new_level() {
        let mut trace = LspTrace::new(TraceValue::Off);
        let params = trace.set_level(TraceValue::Verbose).expect("trace params");

        assert_eq!(params.message, "trace level set to verbose");
    }
}
