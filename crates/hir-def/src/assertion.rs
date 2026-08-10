use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    Ident,
    declaration::DeclarationId,
    expr::{ExprId, data_ty::DataTy, timing_control::TimingControl},
    lower::{BodyStore, LoweringCtx, LoweringStore},
    lower_ident_opt,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct AssertionPort {
    pub name: Option<Ident>,
    pub local: bool,
    pub direction: Option<TokenKind>,
    pub ty: DataTy,
    pub default: Option<ExprId>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PropertySpec {
    pub clocking: Option<TimingControl>,
    pub disable: Option<ExprId>,
    pub expr: ExprId,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PropertyDef {
    pub name: Option<Ident>,
    pub ports: SmallVec<[AssertionPort; 4]>,
    pub variables: SmallVec<[DeclarationId; 2]>,
    pub spec: PropertySpec,
}

pub type PropertyId = Idx<PropertyDef>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct SequenceDef {
    pub name: Option<Ident>,
    pub ports: SmallVec<[AssertionPort; 4]>,
    pub variables: SmallVec<[DeclarationId; 2]>,
    pub expr: ExprId,
}

pub type SequenceId = Idx<SequenceDef>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_property_decl(
        &mut self,
        declaration: ast::PropertyDeclaration,
    ) -> PropertyId {
        let ports = declaration
            .port_list()
            .map(|ports| {
                ports.ports().children().map(|port| self.lower_assertion_port(port)).collect()
            })
            .unwrap_or_default();
        let variables = declaration
            .variables()
            .children()
            .map(|variable| self.lower_body_local_variable_decl(variable))
            .collect();
        let property_spec = declaration.property_spec();
        let spec = PropertySpec {
            clocking: property_spec.clocking().map(|clocking| self.lower_timing_control(clocking)),
            disable: property_spec.disable().map(|disable| self.lower_expr(disable.expr())),
            expr: self.lower_property_expr(property_spec.expr()),
        };
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.properties,
            &mut sources.property_srcs,
            PropertyDef { name: lower_ident_opt(declaration.name()), ports, variables, spec },
            source,
        )
    }

    pub(crate) fn lower_sequence_decl(
        &mut self,
        declaration: ast::SequenceDeclaration,
    ) -> SequenceId {
        let ports = declaration
            .port_list()
            .map(|ports| {
                ports.ports().children().map(|port| self.lower_assertion_port(port)).collect()
            })
            .unwrap_or_default();
        let variables = declaration
            .variables()
            .children()
            .map(|variable| self.lower_body_local_variable_decl(variable))
            .collect();
        let expr = self.lower_sequence_expr(declaration.seq_expr());
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.sequences,
            &mut sources.sequence_srcs,
            SequenceDef { name: lower_ident_opt(declaration.name()), ports, variables, expr },
            source,
        )
    }

    pub(crate) fn lower_default_disable_declaration(
        &mut self,
        declaration: ast::DefaultDisableDeclaration<'_>,
    ) {
        if declaration.default_keyword().map(|token| token.kind())
            != Some(TokenKind::DEFAULT_KEYWORD)
        {
            self.report_invalid(
                declaration.syntax(),
                "default disable declaration is missing its default keyword",
            );
            return;
        }
        if declaration.disable_keyword().map(|token| token.kind())
            != Some(TokenKind::DISABLE_KEYWORD)
        {
            self.report_invalid(
                declaration.syntax(),
                "default disable declaration is missing its disable keyword",
            );
            return;
        }
        if declaration.iff_keyword().map(|token| token.kind()) != Some(TokenKind::IFF_KEYWORD) {
            self.report_invalid(
                declaration.syntax(),
                "default disable declaration is missing its iff keyword",
            );
            return;
        }

        let has_default_disable = {
            let (body, _) = self.store.body();
            body.default_disable.is_some()
        };
        if has_default_disable {
            self.report_invalid(
                declaration.syntax(),
                "scope has more than one default disable declaration",
            );
            return;
        }

        let expr = self.lower_expr(declaration.expr());
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        body.default_disable = Some(expr);
        sources.default_disable_src = Some(source);
    }

    pub(crate) fn lower_assertion_port(&mut self, port: ast::AssertionItemPort) -> AssertionPort {
        AssertionPort {
            name: lower_ident_opt(port.name()),
            local: port.local().is_some(),
            direction: port.direction().map(|direction| direction.kind()),
            ty: self.lower_data_ty(port.type_()),
            default: port.default_value().map(|default| self.lower_property_expr(default.expr())),
        }
    }
}
