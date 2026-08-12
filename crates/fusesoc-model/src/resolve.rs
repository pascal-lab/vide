//! Local-only dependency resolution.
//!
//! Given a set of core roots (directories containing `*.core` files), this
//! module builds a VLNV index and resolves the dependency graph for a given
//! top-level core + target.

use std::collections::{HashMap, HashSet};

use crate::normalize::normalize_core;

use crate::raw::Core;
use crate::vlnv::{Vlnv, VlnvRequirement};

/// An index of locally available cores, keyed by VLN (vendor:library:name).
pub struct CoreIndex {
    /// VLN → list of cores with different versions.
    cores: HashMap<String, Vec<IndexedCore>>,
}

struct IndexedCore {
    vlnv: Vlnv,
    core: Core,
    core_root: utils::paths::AbsPathBuf,
}

/// Result of resolving a dependency graph.
pub struct ResolvedGraph {
    /// All cores in dependency order (top-level first, dependencies after).
    pub cores: Vec<ResolvedGraphCore>,
    /// Errors encountered during resolution.
    pub errors: Vec<ResolutionError>,
}

pub struct ResolvedGraphCore {
    pub vlnv: Vlnv,
    pub core: Core,
    pub core_root: utils::paths::AbsPathBuf,
}

#[derive(Debug)]
pub enum ResolutionError {
    /// A required dependency was not found among local cores.
    MissingDependency(VlnvRequirement),
    /// A dependency has an unsupported feature (generators, providers, etc.).
    Unsupported {
        vlnv: Vlnv,
        feature: String,
        detail: String,
    },
    /// A dependency cycle was detected.
    Cycle(Vec<String>),
    /// Failed to parse a `.core` file.
    ParseError {
        path: utils::paths::AbsPathBuf,
        error: String,
    },
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolutionError::MissingDependency(req) => {
                write!(f, "missing dependency: {}{}", req.relation, req.vlnv)
            }
            ResolutionError::Unsupported { vlnv, feature, detail } => {
                write!(f, "unsupported feature `{feature}` in {vlnv}: {detail}")
            }
            ResolutionError::Cycle(cycle) => {
                write!(f, "dependency cycle: {}", cycle.join(" → "))
            }
            ResolutionError::ParseError { path, error } => {
                write!(f, "failed to parse {}: {error}", path)
            }
        }
    }
}

impl CoreIndex {
    /// Build an index by scanning directories for `*.core` files.
    pub fn from_roots(roots: &[utils::paths::AbsPathBuf]) -> (Self, Vec<ResolutionError>) {
        let mut cores: HashMap<String, Vec<IndexedCore>> = HashMap::new();
        let mut errors = Vec::new();

        for root in roots {
            if std::fs::metadata(root.as_path()).is_err() {
                continue;
            }
            for entry in walk_core_files(root) {
                match load_and_index(&entry) {
                    Ok((vlnv, core)) => {
                        cores.entry(vlnv.vln()).or_default().push(IndexedCore {
                            vlnv,
                            core,
                            core_root: entry
                                .as_path()
                                .parent()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(|| root.clone()),
                        });
                    }
                    Err(e) => {
                        errors.push(ResolutionError::ParseError {
                            path: entry,
                            error: e.to_string(),
                        });
                    }
                }
            }
        }

        (Self { cores }, errors)
    }

    /// Find the best matching core for a VLNV requirement.
    fn find(&self, req: &VlnvRequirement) -> Option<&IndexedCore> {
        let candidates = self.cores.get(&req.vlnv.vln())?;
        // Find all matching, pick the highest version.
        let matching: Vec<_> = candidates.iter().filter(|c| req.matches(&c.vlnv)).collect();
        matching.into_iter().max_by_key(|c| c.vlnv.version.clone())
    }

    /// Resolve the full dependency graph for a top-level core and target.
    ///
    /// The `top_vlnv` identifies the root core.  Dependencies are resolved
    /// transitively via fileset `depend` entries.  Dependency cores use their
    /// `default` target.
    pub fn resolve(
        &self,
        top_vlnv: &Vlnv,
        target: &str,
    ) -> ResolvedGraph {
        let mut errors = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut order: Vec<ResolvedGraphCore> = Vec::new();

        let top_req = VlnvRequirement {
            relation: crate::vlnv::VersionRelation::Equal,
            vlnv: top_vlnv.clone(),
        };
        let Some(top) = self.find(&top_req) else {
            errors.push(ResolutionError::MissingDependency(top_req));
            return ResolvedGraph { cores: order, errors };
        };

        // DFS resolution.
        let mut stack: Vec<(&IndexedCore, String)> = vec![(top, target.to_string())];
        let mut path: Vec<String> = Vec::new();

        while let Some((indexed, tgt)) = stack.pop() {
            let vln_str = indexed.vlnv.vlnv();
            if visited.contains(&vln_str) {
                continue;
            }
            visited.insert(vln_str.clone());

            // Detect cycle.
            if path.contains(&vln_str) {
                errors.push(ResolutionError::Cycle(
                    path.iter().chain(std::iter::once(&vln_str)).cloned().collect(),
                ));
                continue;
            }

            let mut core = indexed.core.clone();
            normalize_core(&mut core);

            // Check for unsupported features used by this target.
            self.check_unsupported(&core, tgt.as_str(), &indexed.vlnv, &mut errors);

            // Collect dependencies from the selected target's filesets.
            let deps = collect_dependencies(&core, tgt.as_str());

            order.push(ResolvedGraphCore {
                vlnv: indexed.vlnv.clone(),
                core: core.clone(),
                core_root: indexed.core_root.clone(),
            });

            path.push(vln_str);

            for dep_str in deps {
                match VlnvRequirement::parse(&dep_str) {
                    Ok(req) => {
                        if let Some(dep_core) = self.find(&req) {
                            stack.push((dep_core, "default".to_string()));
                        } else {
                            errors.push(ResolutionError::MissingDependency(req));
                        }
                    }
                    Err(e) => {
                        errors.push(ResolutionError::ParseError {
                            path: indexed.core_root.clone(),
                            error: format!("invalid dependency `{dep_str}`: {e}"),
                        });
                    }
                }
            }
        }

        ResolvedGraph { cores: order, errors }
    }

    /// Check for features Vide does not support and emit diagnostics.
    fn check_unsupported(
        &self,
        core: &Core,
        target: &str,
        vlnv: &Vlnv,
        errors: &mut Vec<ResolutionError>,
    ) {
        // Check if the selected target uses generators.
        if let Some(tgt) = core.targets.get(target) {
            for gen_name in &tgt.generate {
                if let Some(gen_def) = core.generate.get(gen_name) {
                    errors.push(ResolutionError::Unsupported {
                        vlnv: vlnv.clone(),
                        feature: "generator".to_string(),
                        detail: format!("target `{target}` invokes generator `{gen_name}` ({})", gen_def.generator),
                    });
                }
            }
            // Check if the target uses hooks.
            if let Some(hooks) = &tgt.hooks
                && (!hooks.pre_build.is_empty()
                    || !hooks.post_build.is_empty()
                    || !hooks.pre_run.is_empty()
                    || !hooks.post_run.is_empty())
                {
                    errors.push(ResolutionError::Unsupported {
                        vlnv: vlnv.clone(),
                        feature: "hooks".to_string(),
                        detail: format!("target `{target}` defines build hooks"),
                    });
                }
        }

        // Check for provider — means the core needs to be fetched.
        if let Some(provider) = &core.provider
            && !matches!(provider.name, crate::raw::ProviderKind::Local) {
                errors.push(ResolutionError::Unsupported {
                    vlnv: vlnv.clone(),
                    feature: "provider".to_string(),
                    detail: format!("core uses provider `{}`", provider_name_str(&provider.name)),
                });
            }
    }
}

/// Collect dependency VLNV strings from the selected target's filesets.
fn collect_dependencies(core: &Core, target: &str) -> Vec<String> {
    let Some(tgt) = core.targets.get(target) else {
        return Vec::new();
    };
    let mut deps = Vec::new();
    for fs_name in &tgt.filesets {
        if let Some(fs) = core.filesets.get(fs_name) {
            deps.extend(fs.depend.iter().cloned());
        }
    }
    deps
}

fn provider_name_str(p: &crate::raw::ProviderKind) -> String {
    match p {
        crate::raw::ProviderKind::Github => "github".to_string(),
        crate::raw::ProviderKind::Git => "git".to_string(),
        crate::raw::ProviderKind::Local => "local".to_string(),
        crate::raw::ProviderKind::Opencores => "opencores".to_string(),
        crate::raw::ProviderKind::Svn => "svn".to_string(),
        crate::raw::ProviderKind::Url => "url".to_string(),
        crate::raw::ProviderKind::Other(s) => s.clone(),
    }
}

/// Recursively walk a directory and find all `*.core` files.
fn walk_core_files(dir: &utils::paths::AbsPathBuf) -> Vec<utils::paths::AbsPathBuf> {
    let mut results = Vec::new();
    walk_core_files_inner(dir, &mut results);
    results
}

fn walk_core_files_inner(dir: &utils::paths::AbsPathBuf, results: &mut Vec<utils::paths::AbsPathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir.as_path()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(abs) = utils::paths::abs_path_buf_from_path_buf(path.clone()) {
                // Skip FUSESOC_IGNORE directories.
                let ignore_marker = abs.join("FUSESOC_IGNORE");
                if std::fs::metadata(ignore_marker.as_path()).is_ok() {
                    continue;
                }
                walk_core_files_inner(&abs, results);
            }
        } else if path.extension().is_some_and(|ext| ext == "core")
            && let Some(abs) = utils::paths::abs_path_buf_from_path_buf(path) {
                results.push(abs);
            }
    }
}

fn load_and_index(path: &utils::paths::AbsPathBuf) -> anyhow::Result<(Vlnv, Core)> {
    let core = crate::load_core_file(path)?;
    let vlnv = Vlnv::parse(&core.name).map_err(|e| anyhow::anyhow!(e))?;
    Ok((vlnv, core))
}

#[cfg(test)]
mod tests {
    use super::*;
    use utils::test_support::TestDir;

    fn write_core(dir: &TestDir, name: &str, content: &str) {
        dir.write(format!("{name}.core"), content);
    }

    #[test]
    fn resolves_simple_dependency() {
        let dir = TestDir::new("resolve-simple");
        write_core(
            &dir,
            "top",
            "CAPI=2:\nname: v:l:top:1.0\nfilesets:\n  rtl:\n    files:\n      - top.sv\n    depend:\n      - v:l:dep:1.0\ntargets:\n  default:\n    filesets:\n      - rtl\n    toplevel: top\n",
        );
        write_core(
            &dir,
            "dep",
            "CAPI=2:\nname: v:l:dep:1.0\nfilesets:\n  rtl:\n    files:\n      - dep.sv\ntargets:\n  default:\n    filesets:\n      - rtl\n",
        );

        let (index, parse_errors) = CoreIndex::from_roots(&[dir.path().to_path_buf()]);
        assert!(parse_errors.is_empty(), "{parse_errors:?}");

        let top_vlnv = Vlnv::parse("v:l:top:1.0").unwrap();
        let graph = index.resolve(&top_vlnv, "default");
        assert!(graph.errors.is_empty(), "{:?}", graph.errors);
        assert_eq!(graph.cores.len(), 2);
        assert_eq!(graph.cores[0].vlnv.name, "top");
        assert_eq!(graph.cores[1].vlnv.name, "dep");
    }

    #[test]
    fn reports_missing_dependency() {
        let dir = TestDir::new("resolve-missing");
        write_core(
            &dir,
            "top",
            "CAPI=2:\nname: v:l:top:1.0\nfilesets:\n  rtl:\n    files:\n      - top.sv\n    depend:\n      - v:l:missing:1.0\ntargets:\n  default:\n    filesets:\n      - rtl\n    toplevel: top\n",
        );

        let (index, _) = CoreIndex::from_roots(&[dir.path().to_path_buf()]);
        let top_vlnv = Vlnv::parse("v:l:top:1.0").unwrap();
        let graph = index.resolve(&top_vlnv, "default");
        assert!(graph.cores.len() == 1);
        assert!(graph.errors.iter().any(|e| matches!(e, ResolutionError::MissingDependency(_))));
    }

    #[test]
    fn reports_generator_as_unsupported() {
        let dir = TestDir::new("resolve-gen");
        write_core(
            &dir,
            "top",
            "CAPI=2:\nname: v:l:top:1.0\ngenerate:\n  mygen:\n    generator: some_gen\ngenerators:\n  some_gen:\n    command: gen.py\nfilesets:\n  rtl:\n    files:\n      - top.sv\ntargets:\n  default:\n    filesets:\n      - rtl\n    toplevel: top\n    generate:\n      - mygen\n",
        );

        let (index, _) = CoreIndex::from_roots(&[dir.path().to_path_buf()]);
        let top_vlnv = Vlnv::parse("v:l:top:1.0").unwrap();
        let graph = index.resolve(&top_vlnv, "default");
        assert!(graph
            .errors
            .iter()
            .any(|e| matches!(e, ResolutionError::Unsupported { feature, .. } if feature == "generator")));
    }
}