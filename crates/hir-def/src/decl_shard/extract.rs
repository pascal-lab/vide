use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxKind, SyntaxNode, SyntaxTree, SyntaxTreeOptions, WalkEvent,
    ast::{self, AstNode},
    has_name::HasName,
    has_text_range::HasTextRange,
    token::TokenKindExt,
};
use vfs::FileId;

use super::{Decl, DeclRole, FileDeclShard, ImportSpec, Mention};
use crate::{db::HirDefDb, lower_ident_opt, module::ModuleKind};

pub(super) fn collect(db: &dyn HirDefDb, file_id: FileId) -> FileDeclShard {
    let text = db.file_text(file_id);
    let path = preproc_expand::compilation_plan::source_buffer_path(db, file_id).to_string();
    let name =
        db.file_path(file_id).map(|path| path.to_string()).unwrap_or_else(|| "source".into());
    let context = db.compilation_context_for_file(file_id);
    let options = SyntaxTreeOptions {
        predefines: context.predefines.to_vec(),
        include_paths: Vec::new(),
        include_buffers: Vec::new(),
        expand_includes: false,
        collect_expected_syntax: false,
        expected_syntax_offset: None,
    };
    let tree = SyntaxTree::from_file_in_memory_with_options(&text, &name, &path, &options);
    walk(&tree, &text)
}

fn walk(tree: &SyntaxTree, source_text: &str) -> FileDeclShard {
    let mut decls = Vec::new();
    let mut mentions = Vec::new();
    let mut imports = Vec::new();
    let mut body_depth = 0usize;
    let mut module_depth = 0usize;
    let mut has_compilation_unit_locals = false;
    let mut ordinals = rustc_hash::FxHashMap::<(SmolStr, DeclRole), u32>::default();
    let root = tree.root();
    let preprocessor_independent = tree.preprocessor_trace().include_edges.is_empty()
        && tree.preprocessor_trace().events.is_empty();

    if root.kind() != SyntaxKind::COMPILATION_UNIT {
        return FileDeclShard { preprocessor_independent, ..FileDeclShard::default() };
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
                if body_depth == 0 && module_depth == 0 && ast::Member::can_cast(node.kind()) {
                    if let Some(import) = ast::PackageImportDeclaration::cast(node) {
                        has_compilation_unit_locals = true;
                        imports.extend(import_specs(import));
                    } else if let Some(decl) = member_decl(node, source_text) {
                        if !decl.role.is_design_unit() {
                            has_compilation_unit_locals = true;
                        }
                        let key = (decl.name.clone(), decl.role);
                        let ordinal = ordinals.entry(key).or_insert(0);
                        decls.push(Decl {
                            name: decl.name,
                            role: decl.role,
                            ordinal: *ordinal,
                            header_fingerprint: decl.header_fingerprint,
                        });
                        *ordinal += 1;
                    } else {
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
                }
            }
            WalkEvent::Leave(SyntaxElement::Token(_)) => {}
        }
    }

    FileDeclShard {
        decls: decls.into_boxed_slice(),
        mentions: mentions.into_boxed_slice(),
        imports: imports.into_boxed_slice(),
        preprocessor_independent,
        has_compilation_unit_locals,
    }
}

struct PartialDecl {
    name: SmolStr,
    role: DeclRole,
    header_fingerprint: u64,
}

fn member_decl(node: SyntaxNode<'_>, source_text: &str) -> Option<PartialDecl> {
    let role = decl_role(node)?;
    let name = member_name(node)?;
    if name.is_empty() {
        return None;
    }
    let header_range = ast::ModuleDeclaration::cast(node)
        .map(|item| item.header().syntax())
        .or_else(|| ast::FunctionDeclaration::cast(node).map(|item| item.prototype().syntax()))
        .and_then(|header| header.text_range());
    Some(PartialDecl {
        header_fingerprint: fingerprint(role, &name, header_range, source_text),
        name,
        role,
    })
}

fn decl_role(node: SyntaxNode<'_>) -> Option<DeclRole> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return Some(match ModuleKind::from_ast(module) {
            ModuleKind::Module => DeclRole::Module,
            ModuleKind::Interface => DeclRole::Interface,
            ModuleKind::Package => DeclRole::Package,
            ModuleKind::Program => DeclRole::Program,
        });
    }
    Some(match node.kind() {
        SyntaxKind::CHECKER_DECLARATION => DeclRole::Checker,
        SyntaxKind::COVERGROUP_DECLARATION => DeclRole::Covergroup,
        SyntaxKind::TYPEDEF_DECLARATION | SyntaxKind::FORWARD_TYPEDEF_DECLARATION => {
            DeclRole::Typedef
        }
        SyntaxKind::FUNCTION_DECLARATION | SyntaxKind::TASK_DECLARATION => DeclRole::Subroutine,
        SyntaxKind::PARAMETER_DECLARATION_STATEMENT => DeclRole::Param,
        SyntaxKind::DATA_DECLARATION => DeclRole::Var,
        SyntaxKind::NET_DECLARATION | SyntaxKind::USER_DEFINED_NET_DECLARATION => DeclRole::Net,
        SyntaxKind::EMPTY_MEMBER | SyntaxKind::PACKAGE_IMPORT_DECLARATION => return None,
        _ => DeclRole::Other,
    })
}

fn member_name(node: SyntaxNode<'_>) -> Option<SmolStr> {
    if let Some(module) = ast::ModuleDeclaration::cast(node) {
        return HasName::name(&module).map(|token| token.value_text().to_smolstr());
    }
    if let Some(function) = ast::FunctionDeclaration::cast(node) {
        return HasName::name(&function).map(|token| token.value_text().to_smolstr());
    }
    if let Some(typedef) = ast::TypedefDeclaration::cast(node) {
        return typedef.name().map(|token| token.value_text().to_smolstr());
    }
    if let Some(checker) = ast::CheckerDeclaration::cast(node) {
        return checker.name().map(|token| token.value_text().to_smolstr());
    }
    if let Some(covergroup) = ast::CovergroupDeclaration::cast(node) {
        return covergroup.name().map(|token| token.value_text().to_smolstr());
    }
    None
}

fn import_specs(import: ast::PackageImportDeclaration<'_>) -> Vec<ImportSpec> {
    import
        .items()
        .children()
        .filter_map(|item| {
            let package = lower_ident_opt(item.package())?;
            let imported = item.item()?;
            let item = (imported.kind() != syntax::TokenKind::STAR)
                .then(|| lower_ident_opt(Some(imported)))
                .flatten();
            Some(ImportSpec { package, item })
        })
        .collect()
}

fn is_body_boundary(node: SyntaxNode<'_>) -> bool {
    ast::FunctionDeclaration::can_cast(node.kind()) || ast::ProceduralBlock::can_cast(node.kind())
}

fn fingerprint(
    role: DeclRole,
    name: &SmolStr,
    header_range: Option<utils::line_index::TextRange>,
    source_text: &str,
) -> u64 {
    let mut hasher = FxHasher::default();
    role.hash(&mut hasher);
    name.hash(&mut hasher);
    if let Some(range) = header_range
        && let Some(header) = source_text.get(usize::from(range.start())..usize::from(range.end()))
    {
        header.hash(&mut hasher);
    }
    hasher.finish()
}
