pub mod aggregate;
pub mod block;
pub mod checker;
pub mod covergroup;
pub mod declaration;
pub mod expr;
pub mod file;
pub mod literal;
pub(crate) mod lower;
pub mod macro_file;
pub mod module;
pub mod proc;
pub mod stmt;
pub mod subroutine;
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
    $($src:ty => $id:ty => $field:ident),* $(,)?
) {
    $(
        impl utils::get::Get<$src> for $container {
            type Output = Option<$id>;

            fn get(&self, src: $src) -> Self::Output {
                utils::get::Get::get(&self.$field, src)
            }
        }

        impl utils::get::Get<$id> for $container {
            type Output = Option<$src>;

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

pub(crate) fn alloc_with_optional_source_entry<Src, Input, Hir>(
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Src, Hir>,
    value: Input,
    source: Option<Src>,
) -> Idx<Hir>
where
    Input: Into<Hir>,
    Src: crate::source_map::IsSrc,
{
    let idx = data.alloc(value.into());
    if let Some(source) = source {
        sources.insert(source, idx);
    }
    idx
}

pub(crate) fn alloc_with_source_entry<Src, Input, Hir>(
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Src, Hir>,
    value: Input,
    source: Src,
) -> Idx<Hir>
where
    Input: Into<Hir>,
    Src: crate::source_map::IsSrc,
{
    alloc_with_optional_source_entry(data, sources, value, Some(source))
}

pub(crate) fn alloc_with_source<'ast, Ast, Input, Hir, Src>(
    file_id: crate::file::HirFileId,
    data: &mut Arena<Hir>,
    sources: &mut crate::source_map::SourceMap<Src, Hir>,
    value: Input,
    ast: Ast,
) -> Idx<Hir>
where
    Ast: syntax::ast::AstNode<'ast>,
    Input: Into<Hir>,
    Src: crate::source_map::FromSourceAst<'ast, Ast> + crate::source_map::IsSrc,
{
    let source = crate::source_map::SourceAst::new(file_id, ast).map(Src::from_source_ast);
    alloc_with_optional_source_entry(data, sources, value, source)
}
