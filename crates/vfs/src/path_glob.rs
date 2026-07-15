use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use utils::paths::{AbsPath, AbsPathBuf};

#[derive(Clone)]
pub struct PathGlobMatcher {
    root: AbsPathBuf,
    patterns: Vec<String>,
    matcher: Arc<GlobSet>,
}

impl PathGlobMatcher {
    pub fn new(root: AbsPathBuf, patterns: Vec<String>) -> Result<Self, globset::Error> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &patterns {
            let glob =
                GlobBuilder::new(pattern).literal_separator(true).backslash_escape(true).build()?;
            builder.add(glob);
        }
        let matcher = Arc::new(builder.build()?);

        Ok(Self { root, patterns, matcher })
    }

    pub fn is_match(&self, path: &AbsPath) -> bool {
        let Some(relative) = path.strip_prefix(&self.root) else {
            return false;
        };

        let relative = relative.as_ref().to_string_lossy().replace('\\', "/");
        self.matcher.is_match(relative.as_str())
    }

    pub fn root(&self) -> &AbsPathBuf {
        &self.root
    }

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

impl fmt::Debug for PathGlobMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathGlobMatcher")
            .field("root", &self.root)
            .field("patterns", &self.patterns)
            .finish()
    }
}

impl PartialEq for PathGlobMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.root == other.root && self.patterns == other.patterns
    }
}

impl Eq for PathGlobMatcher {}

impl Hash for PathGlobMatcher {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.root.hash(state);
        self.patterns.hash(state);
    }
}
