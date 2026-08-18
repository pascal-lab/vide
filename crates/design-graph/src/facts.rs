//! Per-file unexpanded design-unit facts.

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::TokenKind;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::unit::{InstantiationRole, UnitId, UnitNode, UnitOrigin};

pub mod extract;

/// One name-like token, unresolved.
///
/// `emitted` is the preprocessor-trace index when the extract tree assigned
/// one. Macro-expanded tokens share display ranges, so later recovery on the
/// authoritative parse needs this identity when the two traces agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub name: smol_str::SmolStr,
    pub kind: TokenKind,
    pub range: TextRange,
    pub emitted: Option<u32>,
}

/// Instantiation type-name token. Primitive instantiations are not recorded.
///
/// `container` is the compilation-unit that directly contains the site.
/// Nested-module bodies leave it empty — those are not CU graph edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiationSite {
    pub file: FileId,
    pub name: smol_str::SmolStr,
    pub range: TextRange,
    pub role: InstantiationRole,
    pub emitted: Option<u32>,
    pub container: Option<UnitId>,
}

/// `import p::x` / `import p::*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSpec {
    pub package: smol_str::SmolStr,
    pub item: Option<smol_str::SmolStr>,
    /// Package-name token in display coordinates.
    pub range: TextRange,
}

/// Left identifier of a non-dot `ScopedName` (`p::y`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRefSite {
    pub name: smol_str::SmolStr,
    pub range: TextRange,
    pub emitted: Option<u32>,
}

/// Position-free CU declaration index. This is what salsa backdates;
/// ranges live on [`Mentions`] and must not enter the global catalog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeclIndex {
    pub units: Box<[DeclUnit]>,
    pub imports: Box<[(SmolStr, Option<SmolStr>)]>,
    pub preprocessor_independent: bool,
    pub has_compilation_unit_locals: bool,
}

/// One CU declaration without source ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclUnit {
    pub id: UnitId,
    pub origin: UnitOrigin,
    pub header_fingerprint: u64,
}

/// Name-like tokens of one file, with a name → offset inverted index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mentions {
    pub entries: Box<[Mention]>,
    by_name: FxHashMap<SmolStr, SmallVec<[u32; 2]>>,
}

impl Mentions {
    pub fn from_entries(entries: Box<[Mention]>) -> Self {
        let mut by_name: FxHashMap<SmolStr, SmallVec<[u32; 2]>> = FxHashMap::default();
        for (index, mention) in entries.iter().enumerate() {
            by_name.entry(mention.name.clone()).or_default().push(index as u32);
        }
        Self { entries, by_name }
    }

    pub fn mentions_name(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn mentions_of(&self, name: &str) -> impl Iterator<Item = &Mention> {
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .map(|&index| &self.entries[index as usize])
    }
}

/// Compact unexpanded slice of one file. No syntax tree, no interned owner.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileFacts {
    pub units: Box<[UnitNode]>,
    pub mentions: Mentions,
    pub imports: Box<[ImportSpec]>,
    pub instantiations: Box<[InstantiationSite]>,
    pub package_refs: Box<[PackageRefSite]>,
    pub preprocessor_independent: bool,
    pub has_compilation_unit_locals: bool,
}

impl FileFacts {
    pub fn decls(&self) -> DeclIndex {
        DeclIndex {
            units: self
                .units
                .iter()
                .map(|unit| DeclUnit {
                    id: unit.id.clone(),
                    origin: unit.origin,
                    header_fingerprint: unit.header_fingerprint,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            imports: self
                .imports
                .iter()
                .map(|import| (import.package.clone(), import.item.clone()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            preprocessor_independent: self.preprocessor_independent,
            has_compilation_unit_locals: self.has_compilation_unit_locals,
        }
    }

    pub fn mentions_name(&self, name: &str) -> bool {
        self.mentions.mentions_name(name)
    }

    pub fn mentions_of(&self, name: &str) -> impl Iterator<Item = &Mention> {
        self.mentions.mentions_of(name)
    }

    pub fn has_compilation_unit_locals(&self) -> bool {
        self.has_compilation_unit_locals
    }

    /// Design-unit whose recorded name token covers `offset`.
    pub fn design_unit_at(&self, offset: TextSize) -> Option<&UnitNode> {
        self.units.iter().find(|unit| unit.name_range.is_some_and(|range| range.contains(offset)))
    }

    pub fn unit(&self, id: UnitId) -> Option<&UnitNode> {
        self.units.iter().find(|unit| unit.id == id)
    }

    pub fn instantiation_at(&self, offset: TextSize) -> Option<&InstantiationSite> {
        self.instantiations.iter().find(|site| site.range.contains(offset))
    }

    pub fn unit_at_name_range(&self, range: TextRange) -> Option<&UnitNode> {
        self.units.iter().find(|unit| unit.name_range == Some(range))
    }

    /// Import package token or `::` left ident covering `offset`.
    pub fn package_token_at(&self, offset: TextSize) -> Option<(smol_str::SmolStr, TextRange)> {
        if let Some(import) = self.imports.iter().find(|import| import.range.contains(offset)) {
            return Some((import.package.clone(), import.range));
        }
        self.package_refs
            .iter()
            .find(|site| site.range.contains(offset))
            .map(|site| (site.name.clone(), site.range))
    }

    /// Whether CU units and import *names* match. Mentions, instantiations,
    /// package-ref sites, and source ranges do not move the structure clock.
    pub fn same_structure(&self, other: &Self) -> bool {
        self.decls() == other.decls()
    }
}
