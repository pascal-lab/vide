//! Cursor classification against live `FileFacts` and a name join.

use smallvec::SmallVec;
use utils::line_index::TextSize;

use crate::{facts::FileFacts, graph::UnitCatalog, unit::UnitId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorHit {
    DeclName {
        unit: UnitId,
        range: utils::line_index::TextRange,
    },
    InstantiationType {
        range: utils::line_index::TextRange,
        targets: SmallVec<[UnitId; 2]>,
    },
    PackageRef {
        name: smol_str::SmolStr,
        range: utils::line_index::TextRange,
        targets: SmallVec<[UnitId; 2]>,
    },
    Other,
}

/// Token shape is a *candidate* graph question. Empty candidates mean this
/// is not a compilation-unit name (`Other`), not a second CU-name path.
pub fn hit_at(facts: &FileFacts, graph: &UnitCatalog, offset: TextSize) -> CursorHit {
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

#[cfg(test)]
mod tests {
    use syntax::SyntaxTree;
    use vfs::FileId;

    use super::{CursorHit, hit_at};
    use crate::{
        facts::extract::from_tree,
        graph::{UnitCatalog, UnitMeta},
        unit::{UnitId, UnitKind, UnitOrigin},
    };

    const FILE: FileId = FileId::from_raw(0);

    fn facts_and_offset(
        text: &str,
        needle: &str,
    ) -> (crate::FileFacts, utils::line_index::TextSize) {
        let tree = SyntaxTree::from_file_in_memory(text, "t.sv", "t.sv");
        let facts = from_tree(FILE, &tree, text);
        let start = text.find(needle).expect(needle);
        (facts, utils::line_index::TextSize::from(start as u32))
    }

    fn graph_with(names: &[(&str, UnitKind)]) -> UnitCatalog {
        let mut graph = UnitCatalog::default();
        for (name, kind) in names {
            let id =
                UnitId { file: FILE, name: smol_str::SmolStr::new(*name), kind: *kind, ordinal: 0 };
            graph.insert(
                id,
                UnitMeta { kind: *kind, origin: UnitOrigin::Source, header_fingerprint: 0 },
            );
        }
        graph
    }

    #[test]
    fn hierarchy_in_module_body_is_instantiation_when_named() {
        let (facts, offset) =
            facts_and_offset("module top;\n  cc_fifo u();\nendmodule\n", "cc_fifo");
        let graph = graph_with(&[("cc_fifo", UnitKind::Module)]);
        assert!(matches!(
            hit_at(&facts, &graph, offset),
            CursorHit::InstantiationType { .. }
        ));
    }

    #[test]
    fn nested_module_instance_is_other() {
        let (facts, offset) = facts_and_offset(
            "module outer;\n  module inner;\n  endmodule\n  inner u();\nendmodule\n",
            "inner u",
        );
        let graph = graph_with(&[("outer", UnitKind::Module)]);
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::Other));
    }

    #[test]
    fn class_scope_left_is_other() {
        let (facts, offset) =
            facts_and_offset("class C; endclass\nmodule m;\n  C::x y;\nendmodule\n", "C::");
        let graph = graph_with(&[("m", UnitKind::Module)]);
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::Other));
    }

    #[test]
    fn import_package_is_package_ref() {
        let text = "import p::*;\nmodule m;\nendmodule\n";
        let tree = SyntaxTree::from_file_in_memory(text, "t.sv", "t.sv");
        let facts = from_tree(FILE, &tree, text);
        let offset = facts.imports[0].range.start();
        let graph = graph_with(&[("p", UnitKind::Package), ("m", UnitKind::Module)]);
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::PackageRef { .. }));
    }

    #[test]
    fn scoped_colon_package_is_package_ref() {
        let (facts, offset) = facts_and_offset("module m;\n  p::y x;\nendmodule\n", "p::");
        let graph = graph_with(&[("p", UnitKind::Package), ("m", UnitKind::Module)]);
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::PackageRef { .. }));
    }

    #[test]
    fn dotted_name_is_other() {
        let (facts, offset) = facts_and_offset("module m;\n  assign x = n.sig;\nendmodule\n", "n.");
        let graph = graph_with(&[("m", UnitKind::Module)]);
        assert!(facts.package_refs.is_empty());
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::Other));
    }

    #[test]
    fn primitive_instantiation_is_other() {
        let (facts, offset) =
            facts_and_offset("module top;\n  and g(o, a, b);\nendmodule\n", "and ");
        let graph = graph_with(&[("top", UnitKind::Module)]);
        assert!(facts.instantiations.is_empty());
        assert!(matches!(hit_at(&facts, &graph, offset), CursorHit::Other));
    }

    #[test]
    fn checker_is_not_a_hierarchy_candidate() {
        let (facts, offset) =
            facts_and_offset("checker c;\nendchecker\nmodule top;\n  c u();\nendmodule\n", "c u");
        let graph = graph_with(&[("c", UnitKind::Checker), ("top", UnitKind::Module)]);
        assert!(
            facts.instantiations.iter().any(|site| site.name == "c"),
            "slang parses `c u()` as hierarchy: {:?}",
            facts.instantiations
        );
        assert!(
            matches!(hit_at(&facts, &graph, offset), CursorHit::Other),
            "Checker is a node, not a Hierarchy candidate"
        );
    }
}
