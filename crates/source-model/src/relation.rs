use crate::ids::{
    EntityId, HirId, IncludeDirectiveId, MacroCallId, MacroExpansionId, OriginId, SourceContextId,
    SourceSelectionId, SpanId,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceRelation {
    Contains {
        parent: EntityId,
        child: EntityId,
    },
    HasSelection {
        entity: EntityId,
        selection: SourceSelectionId,
    },
    ResolvesTo {
        context: SourceContextId,
        reference: EntityId,
        definition: EntityId,
        reason: ResolutionReason,
    },
    Includes {
        context: SourceContextId,
        directive: IncludeDirectiveId,
        included_context: SourceContextId,
    },
    Expands {
        context: SourceContextId,
        call: MacroCallId,
        expansion: MacroExpansionId,
    },
    EmitsToken {
        expansion: MacroExpansionId,
        token: EntityId,
    },
    SpelledFrom {
        generated: SpanId,
        source: SpanId,
        kind: SpellingKind,
    },
    DisplayedAs {
        generated: SpanId,
        display: SpanId,
    },
    HasOrigin {
        entity: EntityId,
        origin: OriginId,
    },
    LowersTo {
        origin: OriginId,
        hir: HirId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolutionReason {
    VisibleDefinition,
    IncludeGuardIfNDef,
    SemanticResolution,
    Builtin,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellingKind {
    Direct,
    MacroBody,
    MacroArgument,
    TokenPaste,
    Stringification,
    Builtin,
    DisplayProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceRelationEndpoint {
    Entity(EntityId),
    Span(SpanId),
    Origin(OriginId),
    Context(SourceContextId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceRelationTarget {
    Entity(EntityId),
    Span(SpanId),
    Origin(OriginId),
    Selection(SourceSelectionId),
}
