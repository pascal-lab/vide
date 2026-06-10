pub mod context;
pub mod domain;
pub mod entity;
pub mod graph;
pub mod ids;
pub mod origin;
pub mod relation;
pub mod resolve;
pub mod span;

pub use context::{SourceContext, SpeculativeReason};
pub use domain::{SourceDomain, SourceUnavailable, VirtualOrigin};
pub use entity::SourceEntity;
pub use graph::{EntityHit, SourceGraph, SourceGraphBuilder};
pub use ids::*;
pub use origin::{
    MacroArgumentTokenIdentity, MacroBodyTokenIdentity, MacroOperationTokenIdentity, SourceOrigin,
    SyntheticReason,
};
pub use relation::{
    ResolutionReason, SourceRelation, SourceRelationEndpoint, SourceRelationTarget, SpellingKind,
};
pub use resolve::{
    ResolvedSourceTarget, SourceBlock, SourceBlockReason, SourceChoice, SourcePurpose,
    SourceRangeResult, SourceTarget, SourceTargetResolution,
};
pub use span::{FilePosition, FileRange, SourceSelection, Span};
