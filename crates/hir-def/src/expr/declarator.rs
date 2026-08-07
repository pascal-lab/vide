use la_arena::{Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};
use utils::define_enum_deriving_from;

use super::{ExprId, data_ty::Dimension};
use crate::{
    Ident,
    ast_id_map::SourceAstId,
    declaration::DeclarationId,
    lower::{LoweringCtx, LoweringStore},
    lower_ident_opt,
    module::port::PortDeclId,
    stmt::StmtId,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Declarator {
    pub name: Option<Ident>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub initializer: Option<ExprId>,
    pub secondary_initializer: Option<ExprId>,
    pub parent: DeclaratorParent,
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum DeclaratorParent {
        PortDeclId,
        DeclarationId, // ParamDecl, NetDecl, DataDecl
        StmtId, // similar to DataDecl
    }
}

pub type DeclId = Idx<Declarator>;

pub(crate) fn empty_decls_range() -> DeclsRange {
    let start = Idx::from_raw(RawIdx::from(0));
    DeclsRange::new(start..start)
}
pub type DeclsRange = IdxRange<Declarator>;

pub type DeclaratorSrc = SourceAstId;

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::Declarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(declarators.children().map(|decl| self.lower_declarator(decl, parent)))
    }

    pub(crate) fn lower_declarator(
        &mut self,
        declarator: ast::Declarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let dimensions =
            declarator.dimensions().children().map(|dim| self.lower_dimension(dim)).collect();
        let initializer = declarator.initializer().map(|init| self.lower_expr(init.expr()));
        let data =
            Declarator { name, dimensions, initializer, secondary_initializer: None, parent };
        let source = self.source_id(declarator.syntax());
        let id = {
            let (declarators, sources) = self.declarators();
            crate::alloc_with_source_entry(declarators, sources, data, source)
        };
        self.record_body_declarator(id);
        id
    }

    pub(crate) fn lower_identifier_names<'a>(
        &mut self,
        identifiers: ast::SeparatedList<'a, ast::IdentifierName<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(identifiers.children().map(|ident| self.lower_identifier_name(ident, parent)))
    }

    fn lower_identifier_name(
        &mut self,
        ident: ast::IdentifierName,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(ident.identifier());
        let data = Declarator {
            name,
            dimensions: SmallVec::new(),
            initializer: None,
            secondary_initializer: None,
            parent,
        };
        let source = self.source_id(ident.syntax());
        let id = {
            let (declarators, sources) = self.declarators();
            crate::alloc_with_source_entry(declarators, sources, data, source)
        };
        self.record_body_declarator(id);
        id
    }

    pub(crate) fn lower_specparam_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::SpecparamDeclarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(
            declarators.children().map(|decl| self.lower_specparam_declarator(decl, parent)),
        )
    }

    fn lower_specparam_declarator(
        &mut self,
        declarator: ast::SpecparamDeclarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let initializer = Some(self.lower_expr(declarator.value_1()));
        let secondary_initializer = declarator.value_2().map(|expr| self.lower_expr(expr));
        let data = Declarator {
            name,
            dimensions: SmallVec::new(),
            initializer,
            secondary_initializer,
            parent,
        };
        let source = self.source_id(declarator.syntax());
        let id = {
            let (declarators, sources) = self.declarators();
            crate::alloc_with_source_entry(declarators, sources, data, source)
        };
        self.record_body_declarator(id);
        id
    }
}

fn decls_range(mut ids: impl Iterator<Item = DeclId>) -> DeclsRange {
    let Some(first) = ids.next() else {
        return empty_decls_range();
    };
    let last = ids.last().unwrap_or(first);
    DeclsRange::new_inclusive(first..=last)
}
