use lsp_types::TraceValue;

use crate::global_state::GlobalState;

// Tracks the client's LSP trace setting. Protocol traffic tracing is handled by
// the client; server-side trace output should be emitted only from explicit
// diagnostic trace points.
#[derive(Debug, Clone)]
pub(crate) struct LspTrace {
    level: TraceValue,
}

impl LspTrace {
    pub(crate) fn new(level: TraceValue) -> Self {
        Self { level }
    }

    pub(crate) fn set_level(&mut self, level: TraceValue) {
        self.level = level;
    }

    #[cfg(test)]
    pub(crate) fn level(&self) -> TraceValue {
        self.level
    }
}

impl GlobalState {
    pub(crate) fn set_lsp_trace(&mut self, level: TraceValue) {
        self.lsp_trace.set_level(level);
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::TraceValue;

    use super::LspTrace;

    #[test]
    fn remembers_initial_trace_level() {
        let trace = LspTrace::new(TraceValue::Messages);

        assert_eq!(trace.level(), TraceValue::Messages);
    }

    #[test]
    fn set_level_updates_trace_level() {
        let mut trace = LspTrace::new(TraceValue::Off);

        trace.set_level(TraceValue::Verbose);

        assert_eq!(trace.level(), TraceValue::Verbose);
    }
}
