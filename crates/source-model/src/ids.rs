macro_rules! source_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            pub fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> u32 {
                self.0
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }
    };
}

source_id!(SourceDomainId);
source_id!(SpanId);
source_id!(SourceSelectionId);
source_id!(EntityId);
source_id!(SourceContextId);
source_id!(OriginId);

source_id!(MacroDefinitionId);
source_id!(MacroReferenceId);
source_id!(MacroCallId);
source_id!(MacroExpansionId);
source_id!(MacroDefinitionIdentity);
source_id!(MacroCallIdentity);
source_id!(MacroExpansionIdentity);
source_id!(MacroParamDefinitionId);
source_id!(MacroParamReferenceId);
source_id!(IncludeDirectiveId);
source_id!(InactiveRegionId);
source_id!(ExpansionTokenId);
source_id!(HirSymbolId);
source_id!(HirReferenceId);
source_id!(SyntaxTokenEntityId);
source_id!(HirId);
