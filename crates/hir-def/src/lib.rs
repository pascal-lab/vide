//! Lowered definitions and name resolution.
//!
//! This crate is the ECS-style definition implementation: arenas, local
//! indexes, container IDs, scopes, source maps, and resolution results live
//! here. It may depend on `preproc-expand`, but must not depend on type
//! inference, semantic adapters, or IDE features. Its identifiers are an
//! explicit workspace-internal interface, not a stable object-oriented
//! facade.

#![feature(decl_macro)]

pub mod aggregate;
pub mod ast_id_map;
pub mod block;
pub mod body;
pub mod checker;
pub mod container;
pub mod covergroup;
pub mod db;
pub mod declaration;
pub mod def_id;
pub mod design_map;
pub mod diagnostics;
pub mod expr;
pub mod file;
pub mod has_source;
pub mod item_tree;
pub mod literal;
pub(crate) mod lower;
pub mod module;
pub mod nameres;
pub mod owner;
pub mod pathres;
pub mod proc;
pub mod region_tree;
pub mod scope;
pub mod source_map;
pub mod source_projection;
pub mod stmt;
pub mod subroutine;
pub mod symbol;
pub mod ty;
pub mod typedef;

pub(crate) macro impl_arena_getters(
    $container:ty;
    $($id:ty => $field:ident => $output:ty),* $(,)?
) {
    $(
        impl utils::get::GetRef<$id> for $container {
            type Output = $output;

            fn get(&self, id: $id) -> &Self::Output {
                utils::get::GetRef::get(&self.$field, id)
            }
        }
    )*
}

pub(crate) macro impl_source_map_getters(
    $container:ty;
    $($id:ty => $field:ident),* $(,)?
) {
    $(
        impl utils::get::Get<$id> for $container {
            type Output = Option<crate::ast_id_map::SourceAstId>;

            fn get(&self, id: $id) -> Self::Output {
                utils::get::Get::get(&self.$field, id)
            }
        }
    )*
}

use la_arena::{Arena, Idx};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{SyntaxToken, TokenKind, ast};

pub type Ident = SmolStr;

pub const DEFAULT_NAME: SmolStr = SmolStr::new_static("unnamed");

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageImport {
    pub package: Ident,
    /// `None` represents `pkg::*`.
    pub item: Option<Ident>,
}

#[inline]
pub fn lower_ident(ident: Option<SyntaxToken>) -> Option<Ident> {
    Some(ident?.value_text().to_smolstr())
}

// If the ident is empty, return None, which may represent a missing identifier.
#[inline]
pub fn lower_ident_opt(ident: Option<SyntaxToken>) -> Option<Ident> {
    let ident = lower_ident(ident)?;
    if ident.is_empty() { None } else { Some(ident) }
}

#[inline]
pub(crate) fn lower_named_label_opt(label: Option<ast::NamedLabel>) -> Option<Ident> {
    let ident = lower_ident(label?.name())?;
    if ident.is_empty() { None } else { Some(ident) }
}

pub(crate) fn lower_package_imports(
    import_decl: ast::PackageImportDeclaration,
) -> Vec<PackageImport> {
    import_decl
        .items()
        .children()
        .filter_map(|item| {
            let package = lower_ident_opt(item.package())?;
            let item = item.item()?;
            let item =
                (item.kind() != TokenKind::STAR).then(|| lower_ident_opt(Some(item))).flatten();
            Some(PackageImport { package, item })
        })
        .collect()
}

pub(crate) fn alloc_with_optional_source_entry<Input, Hir>(
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Hir>,
    value: Input,
    source: Option<crate::ast_id_map::SourceAstId>,
) -> Idx<Hir>
where
    Input: Into<Hir>,
{
    let idx = data.alloc(value.into());
    if let Some(source) = source {
        sources.insert(source, idx);
    }
    idx
}

pub(crate) fn alloc_with_source_entry<Input, Hir>(
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Hir>,
    value: Input,
    source: crate::ast_id_map::SourceAstId,
) -> Idx<Hir>
where
    Input: Into<Hir>,
{
    alloc_with_optional_source_entry(data, sources, value, Some(source))
}

pub(crate) fn alloc_with_source<'ast, Ast, Input, Hir>(
    ast_ids: &crate::ast_id_map::AstIdMap,
    tree: &syntax::SyntaxTree,
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Hir>,
    value: Input,
    ast: Ast,
) -> Idx<Hir>
where
    Ast: syntax::ast::AstNode<'ast>,
    Input: Into<Hir>,
{
    let source = ast_ids.id_of_node_in_tree(tree, ast.syntax());
    alloc_with_optional_source_entry(data, sources, value, source)
}
