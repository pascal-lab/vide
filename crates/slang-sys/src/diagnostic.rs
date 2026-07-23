// TODO: Now our generated diagnostic struct follow the data structure in slang,
// for example, DiagCode(DiagSubsystem::Lexer, index). We mixed up all the
// diagnostic from different subsystem into one enum. This may confused rust
// compiler. So we may need to split the diagnostic into different enum for each
// subsystem.
mod slang_diagnostic {
    include!(concat!(env!("OUT_DIR"), "/diagnostic.rs"));
}
pub use slang_diagnostic::*;
