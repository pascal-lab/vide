use la_arena::Idx;
use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    Ident, alloc_with_source_entry,
    lower::{BodyStore, LoweringCtx, LoweringStore},
    lower_ident_opt,
    subroutine::{Subroutine, lower_subroutine_prototype},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DpiSpec {
    Dpi,
    DpiC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DpiImportProperty {
    Context,
    Pure,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DpiImport {
    pub spec: DpiSpec,
    pub property: Option<DpiImportProperty>,
    pub c_identifier: Option<Ident>,
    pub method: Subroutine,
}

pub type DpiImportId = Idx<DpiImport>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DpiExportKind {
    Function,
    Task,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DpiExport {
    pub spec: DpiSpec,
    pub c_identifier: Option<Ident>,
    pub kind: DpiExportKind,
    pub name: Ident,
}

pub type DpiExportId = Idx<DpiExport>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_dpi_import(&mut self, declaration: ast::DPIImport) -> Option<DpiImportId> {
        let spec = lower_spec(self, declaration.spec_string(), declaration.syntax())?;
        let property = match declaration.property() {
            None => None,
            Some(token) => match token.kind() {
                TokenKind::CONTEXT_KEYWORD => Some(DpiImportProperty::Context),
                TokenKind::PURE_KEYWORD => Some(DpiImportProperty::Pure),
                _ => {
                    self.report_invalid(declaration.syntax(), "DPI import has an invalid property");
                    return None;
                }
            },
        };
        let ast_ids = self.ast_ids.clone();
        let tree = self.tree.clone();
        let Some(method_keyword) = declaration.method().keyword() else {
            self.report_invalid(declaration.syntax(), "DPI import is missing its subroutine kind");
            return None;
        };
        let is_task = match method_keyword.kind() {
            TokenKind::TASK_KEYWORD => true,
            TokenKind::FUNCTION_KEYWORD => false,
            _ => {
                self.report_invalid(
                    declaration.syntax(),
                    "DPI import has an invalid subroutine kind",
                );
                return None;
            }
        };
        let method = lower_subroutine_prototype(
            declaration.method(),
            is_task,
            false,
            |ty| self.lower_data_ty(ty),
            &ast_ids,
            &tree,
        )
        .or_else(|| {
            self.report_invalid(declaration.syntax(), "DPI import has an invalid method prototype");
            None
        })?;
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.dpi_imports,
            &mut sources.dpi_import_srcs,
            DpiImport {
                spec,
                property,
                c_identifier: lower_ident_opt(declaration.c_identifier()),
                method,
            },
            source,
        ))
    }

    pub(crate) fn lower_dpi_export(&mut self, declaration: ast::DPIExport) -> Option<DpiExportId> {
        let spec = lower_spec(self, declaration.spec_string(), declaration.syntax())?;
        let kind = match declaration.function_or_task() {
            Some(token) => match token.kind() {
                TokenKind::FUNCTION_KEYWORD => DpiExportKind::Function,
                TokenKind::TASK_KEYWORD => DpiExportKind::Task,
                _ => {
                    self.report_invalid(
                        declaration.syntax(),
                        "DPI export has an invalid subroutine kind",
                    );
                    return None;
                }
            },
            None => {
                self.report_invalid(
                    declaration.syntax(),
                    "DPI export is missing its subroutine kind",
                );
                return None;
            }
        };
        let Some(name) = lower_ident_opt(declaration.name()) else {
            self.report_invalid(declaration.syntax(), "DPI export is missing its subroutine name");
            return None;
        };
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.dpi_exports,
            &mut sources.dpi_export_srcs,
            DpiExport {
                spec,
                c_identifier: lower_ident_opt(declaration.c_identifier()),
                kind,
                name,
            },
            source,
        ))
    }
}

fn lower_spec<Store: LoweringStore>(
    ctx: &mut LoweringCtx<Store>,
    token: Option<syntax::SyntaxToken<'_>>,
    node: syntax::SyntaxNode<'_>,
) -> Option<DpiSpec> {
    match token.map(|token| token.value_text().to_smolstr()) {
        Some(spec) if spec == SmolStr::new("DPI") => Some(DpiSpec::Dpi),
        Some(spec) if spec == SmolStr::new("DPI-C") => Some(DpiSpec::DpiC),
        Some(_) => {
            ctx.report_invalid(node, "DPI declaration has an invalid specification string");
            None
        }
        None => {
            ctx.report_invalid(node, "DPI declaration is missing its specification string");
            None
        }
    }
}
