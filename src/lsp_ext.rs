// Compatibility shim for code that has not yet migrated to
// `crate::lsp::protocol`. New code should import from
// `crate::lsp::protocol::{from_proto, to_proto, ...}`.
pub(crate) use crate::lsp::protocol::{ext, from_proto, lsp_error, to_proto};
