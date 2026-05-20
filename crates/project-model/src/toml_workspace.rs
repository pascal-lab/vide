use anyhow::Context;
use itertools::Itertools;
use utils::paths::{AbsPathBuf, sort_and_remove_subfolders};

use crate::{macro_def::MacroDef, toml_manifest::TomlManifestSchema};

#[derive(Debug, PartialEq, Eq)]
pub struct TomlWorkspace {
    pub top_modules: Vec<String>,
    pub workspace_root: AbsPathBuf,
    pub macro_defs: MacroDef,
    pub sources: Vec<AbsPathBuf>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
    pub package: Vec<AbsPathBuf>,
    pub is_lib: bool,
    pub configures_semantic_diagnostics: bool,
}

impl TomlWorkspace {
    pub fn load_from_file(toml: &AbsPathBuf, is_lib: bool) -> anyhow::Result<Self> {
        let toml_file =
            std::fs::read_to_string(toml).with_context(|| format!("failed to read {:?}", toml))?;

        let toml_schema: TomlManifestSchema =
            toml::from_str(&toml_file).with_context(|| format!("failed to parse {:?}", toml))?;

        let top_modules = toml_schema.top_modules;
        let workspace_root = toml
            .parent()
            .with_context(|| format!("manifest path has no parent: {toml}"))?
            .to_path_buf();
        let macro_defs = toml_schema.defines;

        let mut exclude = toml_schema
            .exclude
            .into_iter()
            .map(|path| workspace_root.absolutize(path))
            .collect_vec();
        sort_and_remove_subfolders(&mut exclude);

        let include_dirs_was_configured = toml_schema.include_dirs.is_some();
        let mut sources = Vec::new();
        let mut include_dirs = Vec::new();
        let mut package = Vec::new();

        for path in toml_schema.sources.unwrap_or_default() {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                if path.starts_with(&workspace_root) {
                    sources.push(path);
                } else {
                    package.push(path);
                }
            }
        }

        for path in toml_schema.include_dirs.unwrap_or_default() {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                include_dirs.push(path);
            }
        }

        for path in toml_schema.libraries {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                package.push(path);
            }
        }

        sort_and_remove_subfolders(&mut sources);
        sort_and_remove_subfolders(&mut include_dirs);
        sort_and_remove_subfolders(&mut package);

        if include_dirs.is_empty() && !include_dirs_was_configured {
            include_dirs = sources.clone();
        }
        let configures_semantic_diagnostics = !sources.is_empty() || !include_dirs.is_empty();

        Ok(TomlWorkspace {
            top_modules,
            workspace_root,
            macro_defs,
            sources,
            include_dirs,
            exclude,
            package,
            is_lib,
            configures_semantic_diagnostics,
        })
    }

    pub fn from_unconfigured_root(path: &AbsPathBuf, is_lib: bool) -> Self {
        let sources = if is_lib { vec![path.clone()] } else { Vec::new() };
        let include_dirs = sources.clone();
        Self {
            top_modules: Vec::new(),
            workspace_root: path.clone(),
            macro_defs: MacroDef::default(),
            sources,
            include_dirs,
            exclude: vec![],
            package: vec![],
            is_lib,
            configures_semantic_diagnostics: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use utils::test_support::TestDir;

    use super::*;

    #[test]
    fn empty_manifest_uses_syntax_only_default() {
        let root = TestDir::new("empty-manifest");
        let manifest = root.write("vizsla_config.toml", "");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.sources.is_empty());
        assert!(workspace.include_dirs.is_empty());
        assert!(!workspace.configures_semantic_diagnostics);
    }

    #[test]
    fn unconfigured_root_uses_syntax_only_default() {
        let root = TestDir::new("unconfigured-root");

        let workspace = TomlWorkspace::from_unconfigured_root(&root.path().to_path_buf(), false);

        assert!(workspace.sources.is_empty());
        assert!(workspace.include_dirs.is_empty());
        assert!(!workspace.configures_semantic_diagnostics);
    }

    #[test]
    fn configured_empty_sources_do_not_default_to_workspace_root() {
        let root = TestDir::new("empty-sources");
        let manifest = root.write("vizsla_config.toml", "sources = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.sources.is_empty());
    }

    #[test]
    fn excluded_configured_sources_do_not_default_to_workspace_root() {
        let root = TestDir::new("excluded-sources");
        root.create_dir_all("rtl");
        let manifest =
            root.write("vizsla_config.toml", "sources = [\"rtl\"]\nexclude = [\"rtl\"]\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.sources.is_empty());
    }

    #[test]
    fn configured_empty_include_dirs_do_not_default_to_sources() {
        let root = TestDir::new("empty-include-dirs");
        root.create_dir_all("rtl");
        let manifest = root.write("vizsla_config.toml", "sources = [\"rtl\"]\ninclude_dirs = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.include_dirs.is_empty());
    }
}
