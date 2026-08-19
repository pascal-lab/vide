//! Throwaway unexpanded extract. No Trace, no database.

use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, SyntaxTokenWithParent,
    SyntaxTree, WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
    has_text_range::{HasTextRange, HasTextRangeIn},
    token::TokenKindExt,
};
use vfs::FileId;

use super::{FileFacts, ImportSpec, InstantiationSite, Mention, Mentions, PackageRefSite};
use crate::unit::{InstantiationRole, UnitId, UnitKind, UnitNode, UnitOrigin};

/// Extract design-unit facts from an already-built unexpanded tree.
pub fn from_tree(file: FileId, tree: &SyntaxTree, source_text: &str) -> FileFacts {
    walk(file, tree, source_text)
}

/// Compilation-unit design-unit name tokens on an already-built tree.
///
/// Used by the IDE to classify paid-artifact names against an existing
/// preprocessor trace. Does not build a Trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuUnitName {
    pub kind: UnitKind,
    pub name: SmolStr,
    pub emitted: Option<u32>,
}

pub fn cu_unit_names(tree: &SyntaxTree) -> Vec<CuUnitName> {
    let mut names = Vec::new();
    let mut body_depth = 0usize;
    let mut module_depth = 0usize;
    let root = tree.root();
    if root.kind() != SyntaxKind::COMPILATION_UNIT {
        return names;
    }
    for event in root.elem_preorder() {
        match event {
            WalkEvent::Enter(SyntaxElement::Node(node)) => {
                if body_depth == 0
                    && module_depth == 0
                    && ast::Member::can_cast(node.kind())
                    && let Some(kind) = unit_kind(node)
                    && let Some(token) = member_name_token(node)
                {
                    let name = token.value_text();
                    if !name.is_empty() {
                        let with_parent = SyntaxTokenWithParent { parent: node, tok: token };
                        names.push(CuUnitName {
                            kind,
                            name: SmolStr::new(name),
                            emitted: with_parent.preprocessor_trace_emitted_token_index(),
                        });
                    }
                }
                if ast::ModuleDeclaration::can_cast(node.kind()) {
                    module_depth += 1;
                }
                if is_body_boundary(node) {
                    body_depth += 1;
                }
            }
            WalkEvent::Leave(SyntaxElement::Node(node)) => {
                if is_body_boundary(node) {
                    body_depth -= 1;
                }
                if ast::ModuleDeclaration::can_cast(node.kind()) {
                    module_depth -= 1;
                }
            }
            _ => {}
        }
    }
    names
}

/// Kind + name only. Generated units have no `file_text` header to hash.
pub fn unit_fingerprint(kind: UnitKind, name: &SmolStr) -> u64 {
    fingerprint(kind, name, None, "")
}

fn walk(file: FileId, tree: &SyntaxTree, source_text: &str) -> FileFacts {
    let mut units = Vec::new();
    let mut mentions = Vec::new();
    let mut imports = Vec::new();
    let mut instantiations = Vec::new();
    let mut package_refs = Vec::new();
    let mut body_depth = 0usize;
    let mut module_depth = 0usize;
    let mut current_cu: Option<UnitId> = None;
    let mut has_compilation_unit_locals = false;
    let mut ordinals = rustc_hash::FxHashMap::<(SmolStr, UnitKind), u32>::default();
    let preprocessor_independent = syntax::preprocessor_independent(tree);
    let root = tree.root();

    if root.kind() != SyntaxKind::COMPILATION_UNIT {
        return FileFacts { preprocessor_independent, ..FileFacts::default() };
    }

    for event in root.elem_preorder() {
        match event {
            WalkEvent::Enter(SyntaxElement::Token(token)) => {
                if !token.kind().name_like() {
                    continue;
                }
                let Some(range) = token.text_range() else {
                    continue;
                };
                let name = token.tok.value_text();
                if name.is_empty() {
                    continue;
                }
                mentions.push(Mention {
                    name: SmolStr::new(name),
                    kind: token.kind(),
                    range,
                    emitted: token.preprocessor_trace_emitted_token_index(),
                });
            }
            WalkEvent::Enter(SyntaxElement::Node(node)) => {
                if let Some(mut site) = instantiation_at(file, node, module_depth) {
                    if module_depth == 1 {
                        site.container = current_cu.clone();
                    }
                    instantiations.push(site);
                }
                if let Some(spec) = import_at(node) {
                    if body_depth == 0 && module_depth == 0 {
                        has_compilation_unit_locals = true;
                    }
                    imports.extend(spec);
                }
                if let Some(site) = package_ref_at(node) {
                    package_refs.push(site);
                }
                if body_depth == 0 && module_depth == 0 && ast::Member::can_cast(node.kind()) {
                    if ast::PackageImportDeclaration::can_cast(node.kind()) {
                        // Import locals already recorded above.
                    } else if let Some(partial) = member_unit(node, source_text) {
                        if let Some(kind) = partial.kind {
                            if partial.name_range.is_some() {
                                let key = (partial.name.clone(), kind);
                                let ordinal = ordinals.entry(key).or_insert(0);
                                let ordinal_value = *ordinal;
                                *ordinal += 1;
                                let id = UnitId {
                                    file,
                                    name: partial.name,
                                    kind,
                                    ordinal: ordinal_value,
                                };
                                current_cu = Some(id.clone());
                                units.push(UnitNode {
                                    id,
                                    name_range: partial.name_range,
                                    header_range: partial.header_range,
                                    header_fingerprint: partial.header_fingerprint,
                                    origin: UnitOrigin::Source,
                                });
                            }
                        } else {
                            has_compilation_unit_locals = true;
                        }
                    } else if !ast::PackageImportDeclaration::can_cast(node.kind()) {
                        has_compilation_unit_locals = true;
                    }
                }
                if ast::ModuleDeclaration::can_cast(node.kind()) {
                    module_depth += 1;
                }
                if is_body_boundary(node) {
                    body_depth += 1;
                }
            }
            WalkEvent::Leave(SyntaxElement::Node(node)) => {
                if is_body_boundary(node) {
                    body_depth -= 1;
                }
                if ast::ModuleDeclaration::can_cast(node.kind()) {
                    module_depth -= 1;
                    if module_depth == 0 {
                        current_cu = None;
                    }
                }
            }
            WalkEvent::Leave(SyntaxElement::Token(_)) => {}
        }
    }

    FileFacts {
        units: units.into_boxed_slice(),
        mentions: Mentions::from_entries(mentions.into_boxed_slice()),
        imports: imports.into_boxed_slice(),
        instantiations: instantiations.into_boxed_slice(),
        package_refs: package_refs.into_boxed_slice(),
        preprocessor_independent,
        has_compilation_unit_locals,
    }
}

fn instantiation_at(
    file: FileId,
    node: SyntaxNode<'_>,
    module_depth: usize,
) -> Option<InstantiationSite> {
    if let Some(instantiation) = ast::HierarchyInstantiation::cast(node) {
        return instantiation_from_token(
            file,
            instantiation.type_(),
            InstantiationRole::Hierarchy,
            node,
        );
    }
    if ast::PrimitiveInstantiation::can_cast(node.kind()) {
        return None;
    }
    if module_depth != 0 {
        return None;
    }
    let instantiation = ast::CheckerInstantiation::cast(node)?;
    let name = match instantiation.type_() {
        ast::Name::IdentifierName(ident) => ident.identifier(),
        ast::Name::IdentifierSelectName(ident) => ident.identifier(),
        _ => None,
    };
    instantiation_from_token(file, name, InstantiationRole::Checker, node)
}

fn instantiation_from_token(
    file: FileId,
    token: Option<SyntaxToken<'_>>,
    role: InstantiationRole,
    node: SyntaxNode<'_>,
) -> Option<InstantiationSite> {
    let token = token?;
    let range = token.text_range_in(node)?;
    let name = token.value_text();
    if name.is_empty() {
        return None;
    }
    let with_parent = SyntaxTokenWithParent { parent: node, tok: token };
    Some(InstantiationSite {
        file,
        name: SmolStr::new(name),
        range,
        role,
        emitted: with_parent.preprocessor_trace_emitted_token_index(),
        container: None,
    })
}

fn import_at(node: SyntaxNode<'_>) -> Option<Vec<ImportSpec>> {
    let import = ast::PackageImportDeclaration::cast(node)?;
    let specs: Vec<_> = import
        .items()
        .children()
        .filter_map(|item| {
            let package_tok = item.package()?;
            let range = package_tok.text_range_in(node)?;
            let package = package_tok.value_text();
            if package.is_empty() {
                return None;
            }
            let imported = item.item()?;
            let item = (imported.kind() != syntax::TokenKind::STAR)
                .then(|| {
                    let name = imported.value_text();
                    (!name.is_empty()).then(|| SmolStr::new(name))
                })
                .flatten();
            Some(ImportSpec { package: SmolStr::new(package), item, range })
        })
        .collect();
    Some(specs)
}

fn package_ref_at(node: SyntaxNode<'_>) -> Option<PackageRefSite> {
    let scoped = ast::ScopedName::cast(node)?;
    if scoped_uses_dot(scoped) {
        return None;
    }
    let left = match scoped.left() {
        ast::Name::IdentifierName(ident) => ident.identifier()?,
        ast::Name::IdentifierSelectName(ident) => ident.identifier()?,
        _ => return None,
    };
    let range = left.text_range_in(node)?;
    let name = left.value_text();
    if name.is_empty() {
        return None;
    }
    let with_parent = SyntaxTokenWithParent { parent: node, tok: left };
    Some(PackageRefSite {
        name: SmolStr::new(name),
        range,
        emitted: with_parent.preprocessor_trace_emitted_token_index(),
    })
}

fn scoped_uses_dot(scoped: ast::ScopedName<'_>) -> bool {
    scoped
        .syntax()
        .children()
        .filter_map(|elem| elem.as_token())
        .any(|tok| tok.kind() == syntax::Token![.])
}

struct PartialUnit {
    name: SmolStr,
    kind: Option<UnitKind>,
    header_fingerprint: u64,
    name_range: Option<utils::line_index::TextRange>,
    header_range: Option<utils::line_index::TextRange>,
}

fn member_unit(node: SyntaxNode<'_>, source_text: &str) -> Option<PartialUnit> {
    let kind = unit_kind(node);
    if kind.is_none() && !is_cu_local_member(node) {
        return None;
    }
    let (name, name_range) = member_name(node).unwrap_or_else(|| (SmolStr::new(""), None));
    if kind.is_some() && name.is_empty() {
        return None;
    }
    let header_range = ast::ModuleDeclaration::cast(node)
        .map(|item| item.header().syntax())
        .and_then(|header| header.text_range());
    let fingerprint_kind = kind.unwrap_or(UnitKind::Module);
    Some(PartialUnit {
        header_fingerprint: fingerprint(fingerprint_kind, &name, header_range, source_text),
        name,
        kind,
        name_range,
        header_range,
    })
}

fn unit_kind(node: SyntaxNode<'_>) -> Option<UnitKind> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return Some(kind_from_module(module));
    }
    match node.kind() {
        SyntaxKind::CHECKER_DECLARATION => Some(UnitKind::Checker),
        SyntaxKind::COVERGROUP_DECLARATION => Some(UnitKind::Covergroup),
        _ => None,
    }
}

fn kind_from_module(decl: ast::ModuleDeclaration<'_>) -> UnitKind {
    if decl.as_package_declaration().is_some() {
        UnitKind::Package
    } else if decl.as_interface_declaration().is_some() {
        UnitKind::Interface
    } else if decl.as_program_declaration().is_some() {
        UnitKind::Program
    } else {
        UnitKind::Module
    }
}

fn is_cu_local_member(node: SyntaxNode<'_>) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::TYPEDEF_DECLARATION
            | SyntaxKind::FORWARD_TYPEDEF_DECLARATION
            | SyntaxKind::FUNCTION_DECLARATION
            | SyntaxKind::TASK_DECLARATION
            | SyntaxKind::PARAMETER_DECLARATION_STATEMENT
            | SyntaxKind::DATA_DECLARATION
            | SyntaxKind::NET_DECLARATION
            | SyntaxKind::USER_DEFINED_NET_DECLARATION
    ) || (!ast::EmptyMember::can_cast(node.kind())
        && !ast::PackageImportDeclaration::can_cast(node.kind())
        && ast::Member::can_cast(node.kind()))
}

fn member_name(node: SyntaxNode<'_>) -> Option<(SmolStr, Option<utils::line_index::TextRange>)> {
    let token = member_name_token(node)?;
    Some((token.value_text().to_smolstr(), token.text_range_in(node)))
}

fn member_name_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return HasName::name(&module);
    }
    if let Some(function) = ast::FunctionDeclaration::cast(node) {
        return HasName::name(&function);
    }
    if let Some(typedef) = ast::TypedefDeclaration::cast(node) {
        return typedef.name();
    }
    if let Some(checker) = ast::CheckerDeclaration::cast(node) {
        return checker.name();
    }
    if let Some(covergroup) = ast::CovergroupDeclaration::cast(node) {
        return covergroup.name();
    }
    None
}

fn is_body_boundary(node: SyntaxNode<'_>) -> bool {
    ast::FunctionDeclaration::can_cast(node.kind()) || ast::ProceduralBlock::can_cast(node.kind())
}

fn fingerprint(
    kind: UnitKind,
    name: &SmolStr,
    header_range: Option<utils::line_index::TextRange>,
    source_text: &str,
) -> u64 {
    let mut hasher = FxHasher::default();
    kind.hash(&mut hasher);
    name.hash(&mut hasher);
    if let Some(range) = header_range
        && let Some(header) = source_text.get(usize::from(range.start())..usize::from(range.end()))
    {
        header.hash(&mut hasher);
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use syntax::SyntaxTree;
    use vfs::FileId;

    use super::from_tree;
    use crate::unit::{InstantiationRole, UnitKind};

    const FILE: FileId = FileId::from_raw(0);

    fn facts(text: &str) -> crate::FileFacts {
        let tree = SyntaxTree::from_file_in_memory(text, "t.sv", "t.sv");
        from_tree(FILE, &tree, text)
    }

    #[test]
    fn plain_module_is_preprocessor_independent() {
        let facts = facts("module m;\nendmodule\n");
        assert!(facts.preprocessor_independent);
        assert_eq!(facts.units.len(), 1);
        assert_eq!(facts.units[0].id.kind, UnitKind::Module);
        assert_eq!(facts.units[0].id.name, "m");
        assert_eq!(facts.units[0].id.ordinal, 0);
    }

    #[test]
    fn define_is_preprocessor_activity() {
        assert!(!facts("`define W 8\nmodule m;\nendmodule\n").preprocessor_independent);
    }

    #[test]
    fn include_is_preprocessor_activity() {
        assert!(!facts("`include \"a.svh\"\nmodule m;\nendmodule\n").preprocessor_independent);
    }

    #[test]
    fn ifdef_is_preprocessor_activity() {
        assert!(!facts("`ifdef W\nmodule m;\nendmodule\n`endif\n").preprocessor_independent);
    }

    #[test]
    fn macro_usage_is_preprocessor_activity() {
        assert!(
            !facts("module m;\n  logic [`UNKNOWN-1:0] x;\nendmodule\n").preprocessor_independent
        );
    }

    #[test]
    fn module_header_range_excludes_the_body() {
        let text = "module top #(parameter int W = 1);\n  wire unused;\nendmodule\n";
        let facts = facts(text);
        let header = facts.units[0].header_range.expect("module header");
        let header = &text[usize::from(header.start())..usize::from(header.end())];
        assert!(header.contains("module top"), "{header}");
        assert!(header.contains("parameter int W = 1"), "{header}");
        assert!(!header.contains("wire unused"), "{header}");
    }

    #[test]
    fn nested_module_is_not_a_unit() {
        let facts = facts("module outer;\n  module inner;\n  endmodule\nendmodule\n");
        assert_eq!(facts.units.len(), 1);
        assert_eq!(facts.units[0].id.name, "outer");
    }

    #[test]
    fn hierarchy_instantiation_is_recorded_inside_a_module() {
        let facts = facts("module top;\n  cc_fifo u();\nendmodule\n");
        assert_eq!(facts.instantiations.len(), 1);
        assert_eq!(facts.instantiations[0].name, "cc_fifo");
        assert_eq!(facts.instantiations[0].role, InstantiationRole::Hierarchy);
        assert_eq!(
            facts.instantiations[0].container.as_ref().map(|id| id.name.as_str()),
            Some("top")
        );
    }

    #[test]
    fn nested_instantiation_is_not_a_cu_container_edge() {
        let facts = facts(
            "module outer;\n  module inner;\n    leaf u();\n  endmodule\n  child v();\nendmodule\n",
        );
        let child = facts.instantiations.iter().find(|site| site.name == "child").expect("child");
        let leaf = facts.instantiations.iter().find(|site| site.name == "leaf").expect("leaf");
        assert_eq!(child.container.as_ref().map(|id| id.name.as_str()), Some("outer"));
        assert!(leaf.container.is_none(), "{leaf:?}");
    }

    #[test]
    fn two_cu_modules_keep_distinct_instantiation_containers() {
        let facts = facts("module a;\n  b u();\nendmodule\nmodule c;\n  d v();\nendmodule\n");
        let b = facts.instantiations.iter().find(|site| site.name == "b").expect("b");
        let d = facts.instantiations.iter().find(|site| site.name == "d").expect("d");
        assert_eq!(b.container.as_ref().map(|id| id.name.as_str()), Some("a"));
        assert_eq!(d.container.as_ref().map(|id| id.name.as_str()), Some("c"));
    }

    #[test]
    fn primitive_instantiation_is_not_a_graph_site() {
        let facts = facts("module top;\n  and g(o, a, b);\nendmodule\n");
        assert!(facts.instantiations.is_empty(), "{:?}", facts.instantiations);
    }

    #[test]
    fn import_records_package_range() {
        let facts = facts("import p::*;\nmodule m;\nendmodule\n");
        assert_eq!(facts.imports.len(), 1);
        assert_eq!(facts.imports[0].package, "p");
        assert!(facts.imports[0].item.is_none());
        assert!(facts.has_compilation_unit_locals);
        assert!(facts.package_token_at(facts.imports[0].range.start()).is_some());
    }

    #[test]
    fn scoped_colon_left_is_a_package_ref() {
        let facts = facts("module m;\n  p::y x;\nendmodule\n");
        assert!(facts.package_refs.iter().any(|site| site.name == "p"), "{:?}", facts.package_refs);
    }

    #[test]
    fn dotted_name_is_not_a_package_ref() {
        let facts = facts("module m;\n  assign x = n.sig;\nendmodule\n");
        assert!(facts.package_refs.is_empty(), "{:?}", facts.package_refs);
    }

    #[test]
    fn non_du_cu_member_sets_locals_and_is_not_a_unit() {
        let facts = facts("typedef logic t;\nmodule m;\nendmodule\n");
        assert!(facts.has_compilation_unit_locals);
        assert_eq!(facts.units.len(), 1);
        assert_eq!(facts.units[0].id.name, "m");
    }
}
