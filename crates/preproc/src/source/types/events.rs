use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocError {
    MissingRootSource,
    MissingEventRange { source_order: usize, kind: MacroEventKind },
    MissingEvent { event_id: u32 },
}
