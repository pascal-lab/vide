use la_arena::{Arena, Idx};
use syntax::{TokenKind, ast};

use super::{Ident, lower_ident_opt};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PackageImport {
    pub package: Option<Ident>,
    pub item: PackageImportName,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PackageImportName {
    Wildcard,
    Name(Ident),
}

pub type PackageImportId = Idx<PackageImport>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PackageExport {
    pub package: Option<Ident>,
    pub item: PackageExportName,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PackageExportName {
    Wildcard,
    Name(Ident),
    AllImports,
}

pub type PackageExportId = Idx<PackageExport>;

pub(crate) fn lower_package_imports(
    import: ast::PackageImportDeclaration,
    imports: &mut Arena<PackageImport>,
) {
    for item in import.items().children() {
        let package = lower_ident_opt(item.package());
        let Some(item_token) = item.item() else {
            continue;
        };
        let item = if item_token.kind() == TokenKind::STAR {
            PackageImportName::Wildcard
        } else {
            let Some(name) = lower_ident_opt(Some(item_token)) else {
                continue;
            };
            PackageImportName::Name(name)
        };
        imports.alloc(PackageImport { package, item });
    }
}

pub(crate) fn lower_package_exports(
    export: ast::PackageExportDeclaration,
    exports: &mut Arena<PackageExport>,
) {
    for item in export.items().children() {
        let package = lower_ident_opt(item.package());
        let Some(item_token) = item.item() else {
            continue;
        };
        let item = if item_token.kind() == TokenKind::STAR {
            PackageExportName::Wildcard
        } else {
            let Some(name) = lower_ident_opt(Some(item_token)) else {
                continue;
            };
            PackageExportName::Name(name)
        };
        exports.alloc(PackageExport { package, item });
    }
}

pub(crate) fn lower_package_export_all(
    _export: ast::PackageExportAllDeclaration,
    exports: &mut Arena<PackageExport>,
) {
    exports.alloc(PackageExport { package: None, item: PackageExportName::AllImports });
}
