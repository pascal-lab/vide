use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticProvenance {
    SourceToken {
        file_id: FileId,
        range: TextRange,
    },
    MacroBody {
        call: MacroCall,
        definition_id: MacroDefinitionId,
        file_id: FileId,
        range: TextRange,
    },
    MacroArgument {
        call: MacroCall,
        argument_index: usize,
        file_id: FileId,
        range: TextRange,
    },
    VirtualExpansion {
        file_id: FileId,
        range: TextRange,
    },
    Builtin {
        call: MacroCall,
        name: SmolStr,
    },
    Unavailable(PreprocUnavailable),
}
