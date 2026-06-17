use super::*;

pub(in crate::preproc) enum TokenProvenance {
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
    Predefine,
    Builtin {
        name: SmolStr,
        call: MacroCall,
    },
    TokenPaste,
    Stringification,
    Unavailable,
}

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
