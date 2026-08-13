//! Expansion of a resolved dependency graph into a flat [`ResolvedProject`].
//!
//! This is the neutral, tool-agnostic output that `project-model` can adapt
//! into `Workspace` and `CompilationProfile`.

use utils::paths::AbsPathBuf;

use crate::{normalize::effective_defines, raw::Fileset, resolve::ResolvedGraph, vlnv::Vlnv};

/// A fully resolved project — flat file list, include dirs, defines, tops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    /// All source files in dependency order.
    pub files: Vec<ResolvedFile>,
    /// Include directories (absolute paths).
    pub include_dirs: Vec<AbsPathBuf>,
    /// Global defines (from file-level define attributes, accumulated).
    pub defines: Vec<(String, String)>,
    /// Top-level module names.
    pub top_modules: Vec<String>,
    /// The cores that contributed to this project.
    pub cores: Vec<ResolvedCore>,
}

/// A resolved source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    pub path: AbsPathBuf,
    pub file_type: String,
    pub is_include_file: bool,
    pub include_path: Option<AbsPathBuf>,
    pub defines: Vec<(String, String)>,
    pub logical_name: Option<String>,
}

/// A core that contributed to the resolved project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCore {
    pub vlnv: Vlnv,
    pub core_root: AbsPathBuf,
    pub core_file: AbsPathBuf,
}

/// Expand a resolved dependency graph into a flat project.
///
/// Only Verilog/SystemVerilog source files are included.  Files with other
/// types (constraints, memory init, etc.) are skipped — Vide is a language
/// server, not a build tool.
pub fn expand(graph: &ResolvedGraph, top_target: &str) -> ResolvedProject {
    let mut files = Vec::new();
    let mut include_dirs = Vec::new();
    let mut defines = Vec::new();
    let mut top_modules = Vec::new();
    let mut cores = Vec::new();

    // Process in reverse order so dependencies come before dependents.
    for gc in graph.cores.iter().rev() {
        let core = &gc.core;
        let core_root = &gc.core_root;
        // Top-level core uses the requested target; dependencies use "default".
        let is_top = graph.cores.first().map(|c| c.vlnv.vlnv()) == Some(gc.vlnv.vlnv());
        let target = if is_top { top_target } else { "default" };

        let Some(tgt) = core.targets.get(target) else {
            continue;
        };

        // Top-level modules.
        if is_top {
            top_modules.extend(tgt.top_modules());
        }

        // Expand filesets.
        for fs_name in &tgt.filesets {
            let Some(fs) = core.filesets.get(fs_name) else {
                continue;
            };
            expand_fileset(fs, core_root, &mut files, &mut include_dirs, &mut defines);
        }

        cores.push(ResolvedCore {
            vlnv: gc.vlnv.clone(),
            core_root: gc.core_root.clone(),
            core_file: gc.core_root.join(format!("{}.core", gc.vlnv.name)),
        });
    }

    // Deduplicate include dirs.
    include_dirs.sort();
    include_dirs.dedup();

    ResolvedProject { files, include_dirs, defines, top_modules, cores }
}

/// Expand a single fileset into files, include dirs, and defines.
fn expand_fileset(
    fs: &Fileset,
    core_root: &AbsPathBuf,
    files: &mut Vec<ResolvedFile>,
    include_dirs: &mut Vec<AbsPathBuf>,
    defines: &mut Vec<(String, String)>,
) {
    for entry in &fs.files {
        let path_str = entry.path();
        let abs_path = core_root.join(path_str);

        let attrs = entry.attributes();

        let file_type = attrs
            .and_then(|a| a.file_type.clone())
            .or_else(|| fs.file_type.clone())
            .unwrap_or_default();

        // Only include Verilog/SystemVerilog sources.
        if !is_verilog_source(&file_type) {
            continue;
        }

        let is_include_file = attrs.map(|a| a.is_include_file).unwrap_or(false);

        let include_path = attrs.and_then(|a| {
            if let Some(ip) = &a.include_path {
                Some(core_root.join(ip))
            } else if a.is_include_file {
                abs_path.as_path().parent().map(|p| p.to_path_buf())
            } else {
                None
            }
        });

        if let Some(ip) = &include_path {
            include_dirs.push(ip.clone());
        }

        let file_defines = effective_defines(entry);
        defines.extend(file_defines.iter().cloned());

        files.push(ResolvedFile {
            path: abs_path,
            file_type,
            is_include_file,
            include_path,
            defines: file_defines,
            logical_name: attrs
                .and_then(|a| a.logical_name.clone())
                .or_else(|| fs.logical_name.clone()),
        });
    }
}

/// Check if a file type is Verilog or SystemVerilog.
fn is_verilog_source(file_type: &str) -> bool {
    let ft = file_type.to_ascii_lowercase();
    ft.contains("verilog")
}
