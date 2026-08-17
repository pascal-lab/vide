//! Per-file unexpanded design-unit facts.

use syntax::TokenKind;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::unit::{InstantiationRole, UnitId, UnitNode};

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiationSite {
    pub file: FileId,
    pub name: smol_str::SmolStr,
    pub range: TextRange,
    pub role: InstantiationRole,
    pub emitted: Option<u32>,
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

/// Compact unexpanded slice of one file. No syntax tree, no interned owner.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileFacts {
    pub units: Box<[UnitNode]>,
    pub mentions: Box<[Mention]>,
    pub imports: Box<[ImportSpec]>,
    pub instantiations: Box<[InstantiationSite]>,
    pub package_refs: Box<[PackageRefSite]>,
    pub preprocessor_independent: bool,
    pub has_compilation_unit_locals: bool,
}

impl FileFacts {
    pub fn mentions_name(&self, name: &str) -> bool {
        self.mentions.iter().any(|mention| mention.name == name)
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
        self.has_compilation_unit_locals == other.has_compilation_unit_locals
            && self.preprocessor_independent == other.preprocessor_independent
            && import_names_equal(&self.imports, &other.imports)
            && self.units.len() == other.units.len()
            && self.units.iter().zip(other.units.iter()).all(|(left, right)| {
                left.id.name == right.id.name
                    && left.id.kind == right.id.kind
                    && left.id.ordinal == right.id.ordinal
                    && left.header_fingerprint == right.header_fingerprint
                    && left.origin == right.origin
            })
    }
}

fn import_names_equal(left: &[ImportSpec], right: &[ImportSpec]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right.iter()).all(|(a, b)| a.package == b.package && a.item == b.item)
}
