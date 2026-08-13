//! FuseSoC CLI integration for Vide project loading.
//!
//! FuseSoC owns CAPI2 parsing, dependency resolution, target expansion,
//! providers, and generators. This crate only models the EDAM metadata that
//! FuseSoC emits for an IDE to consume.

use utils::paths::AbsPathBuf;

pub mod cli;

/// A fully resolved FuseSoC project as represented by EDAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    pub files: Vec<ResolvedFile>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub defines: Vec<(String, String)>,
    pub top_modules: Vec<String>,
    pub cores: Vec<ResolvedCore>,
}

/// A source or include file from EDAM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub path: AbsPathBuf,
    pub file_type: String,
    pub is_include_file: bool,
    pub include_path: Option<AbsPathBuf>,
    pub defines: Vec<(String, String)>,
    pub logical_name: Option<String>,
}

/// A core that contributed to the EDAM project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCore {
    pub name: String,
    pub core_root: AbsPathBuf,
    pub core_file: AbsPathBuf,
}
