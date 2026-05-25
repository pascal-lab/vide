use triomphe::Arc;
use utils::paths::AbsPathBuf;

use crate::source_root::SourceRootId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationProfileId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocessConfig {
    pub predefines: Vec<String>,
    pub include_dirs: Vec<AbsPathBuf>,
}

impl PreprocessConfig {
    pub fn include_dir_strings(&self) -> Vec<String> {
        self.include_dirs.iter().map(ToString::to_string).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationProfile {
    pub source_roots: Vec<SourceRootId>,
    pub top_modules: Vec<String>,
    pub preprocess: PreprocessConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectConfig {
    root_profiles: Vec<Option<CompilationProfileId>>,
    profiles: Vec<CompilationProfile>,
}

impl ProjectConfig {
    pub fn new(
        root_profiles: Vec<Option<CompilationProfileId>>,
        profiles: Vec<CompilationProfile>,
    ) -> Self {
        Self { root_profiles, profiles }
    }

    pub fn profile_for_root(&self, root_id: SourceRootId) -> Option<CompilationProfileId> {
        self.root_profiles.get(root_id.0 as usize).copied().flatten()
    }

    pub fn profile(&self, profile_id: CompilationProfileId) -> Option<&CompilationProfile> {
        self.profiles.get(profile_id.0 as usize)
    }

    pub fn root_profile_count(&self) -> usize {
        self.root_profiles.len()
    }

    pub fn has_compilation_profiles(&self) -> bool {
        !self.profiles.is_empty()
    }

    pub fn preprocess_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> PreprocessConfig {
        profile_id
            .and_then(|profile_id| self.profile(profile_id))
            .map(|profile| profile.preprocess.clone())
            .unwrap_or_default()
    }
}

pub type SharedProjectConfig = Arc<ProjectConfig>;
