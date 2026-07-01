pub mod project;
pub mod source_db;
pub mod source_root;

pub use project::{
    CompilationProfile, CompilationProfileId, Predefine, PredefineSource, PreprocessConfig,
    ProjectConfig, SharedProjectConfig,
};
pub use source_db::SourceFileKind;
pub use source_root::{
    SourceRoot, SourceRootConfig, SourceRootDiagnosticScope, SourceRootId, SourceRootRole,
};
