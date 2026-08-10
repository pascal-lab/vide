use la_arena::Idx;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    Ident, alloc_with_source_entry,
    expr::data_ty::DataTy,
    lower::{BodyStore, LoweringCtx, LoweringStore},
    lower_ident_opt,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetTypeDecl {
    pub name: Ident,
    pub ty: DataTy,
    pub with_function: Option<Ident>,
}

pub type NetTypeDeclId = Idx<NetTypeDecl>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_net_type_decl(
        &mut self,
        declaration: ast::NetTypeDeclaration,
    ) -> Option<NetTypeDeclId> {
        if declaration.keyword().map(|token| token.kind()) != Some(TokenKind::NET_TYPE_KEYWORD) {
            self.report_invalid(
                declaration.syntax(),
                "net type declaration is missing its nettype keyword",
            );
            return None;
        }
        let Some(name) = lower_ident_opt(declaration.name()) else {
            self.report_invalid(declaration.syntax(), "net type declaration is missing its name");
            return None;
        };
        let with_function = match declaration.with_function() {
            None => None,
            Some(function) => {
                let name = match function.name() {
                    ast::Name::IdentifierName(name) => lower_ident_opt(name.identifier()),
                    _ => None,
                };
                let Some(name) = name else {
                    self.report_invalid(
                        function.syntax(),
                        "net type declaration has an invalid resolution function name",
                    );
                    return None;
                };
                Some(name)
            }
        };
        let ty = self.lower_data_ty(declaration.type_());
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.net_type_decls,
            &mut sources.net_type_decl_srcs,
            NetTypeDecl { name, ty, with_function },
            source,
        ))
    }
}
