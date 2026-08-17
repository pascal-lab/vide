//! Cursor classification against live `FileFacts` and a name join.

use smallvec::SmallVec;
use utils::line_index::TextSize;
use vfs::FileId;

use crate::{facts::FileFacts, graph::DesignGraph, unit::UnitId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorHit {
    DeclName {
        unit: UnitId,
        range: utils::line_index::TextRange,
    },
    InstantiationType {
        range: utils::line_index::TextRange,
        targets: SmallVec<[UnitId; 1]>,
    },
    PackageRef {
        name: smol_str::SmolStr,
        range: utils::line_index::TextRange,
        targets: SmallVec<[UnitId; 1]>,
    },
    Other,
}

/// Token shape is a *candidate* graph question. Empty candidates mean this
/// is not a compilation-unit name (`Other`), not a second CU-name path.
pub fn hit_at(
    facts: &FileFacts,
    graph: &DesignGraph,
    _file: FileId,
    offset: TextSize,
) -> CursorHit {
    if let Some(decl) = facts.design_unit_at(offset) {
        let range = decl.name_range.expect("design_unit_at only returns ranged decls");
        return CursorHit::DeclName { unit: decl.id.clone(), range };
    }
    if let Some(site) = facts.instantiation_at(offset) {
        let targets = graph.candidates(&site.name, site.role);
        if !targets.is_empty() {
            return CursorHit::InstantiationType { range: site.range, targets };
        }
    }
    if let Some((name, range)) = facts.package_token_at(offset) {
        let targets = graph.packages_named(&name).into_vec();
        if !targets.is_empty() {
            return CursorHit::PackageRef { name, range, targets };
        }
    }
    CursorHit::Other
}
