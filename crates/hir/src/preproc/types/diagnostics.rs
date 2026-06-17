use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticProvenance {
    SourceToken {
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroBody {
        call: MacroCall,
        definition_id: MacroDefinitionId,
        source: MappedPreprocSource,
        range: TextRange,
    },
    MacroArgument {
        call: MacroCall,
        argument_index: usize,
        source: MappedPreprocSource,
        range: TextRange,
    },
    VirtualExpansion {
        source: MappedPreprocSource,
        range: TextRange,
    },
    Builtin {
        call: MacroCall,
        name: SmolStr,
    },
    Unavailable(PreprocUnavailable),
}
