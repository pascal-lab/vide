use la_arena::Idx;
use syntax::{TokenKind, ast};
use utils::define_enum_deriving_from;

use super::expr::{
    data_ty::{BuiltinDataTy, BuiltinDataTyId, DataTy, IntKind},
    declarator::{DeclsRange, empty_decls_range},
    timing_control::DelayControl,
};
use crate::{
    alloc_with_source,
    ast_id_map::SourceAstId,
    lower::{LoweringCtx, LoweringStore},
    ty::{DriveStrength, NetKind, Strength, lower_drive_strength, lower_net_kind, lower_strength},
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum Declaration {
        DataDecl,
        NetDecl,
        ParamDecl,
        GenvarDecl,
        SpecparamDecl,
    }
}

pub type DeclarationId = Idx<Declaration>;

pub type DeclarationSrc = SourceAstId;

impl Declaration {
    pub fn decls(&self) -> DeclsRange {
        match self {
            Declaration::DataDecl(data_decl) => data_decl.decls.clone(),
            Declaration::NetDecl(net_decl) => net_decl.decls.clone(),
            Declaration::ParamDecl(param_decl) => param_decl.decls.clone(),
            Declaration::GenvarDecl(genvar_decl) => genvar_decl.decls.clone(),
            Declaration::SpecparamDecl(specparam_decl) => specparam_decl.decls.clone(),
        }
    }

    pub fn ty(&self) -> DataTy {
        match self {
            Declaration::DataDecl(data_decl) => data_decl.ty.clone(),
            Declaration::NetDecl(net_decl) => net_decl.ty.clone(),
            Declaration::ParamDecl(param_decl) => param_decl.ty.clone(),
            Declaration::GenvarDecl(genvar_decl) => genvar_decl.ty.clone(),
            Declaration::SpecparamDecl(specparam_decl) => specparam_decl.ty.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataDecl {
    pub ty: DataTy,
    pub const_kw: bool,
    pub var_kw: bool,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub ty: DataTy,
    pub net_kind: Option<NetKind>,
    pub delay: Option<DelayControl>,
    pub strength: Option<NetStrength>,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum NetStrength {
    Pull(Strength),
    Drive(DriveStrength),
    Charge(Strength),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParamDecl {
    pub ty: DataTy,
    pub kind: ParamDeclKind,
    pub is_port: bool,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ParamDeclKind {
    Parameter,
    LocalParam,
}

impl ParamDeclKind {
    pub fn is_overridable(self) -> bool {
        matches!(self, Self::Parameter)
    }

    pub fn keyword(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::LocalParam => "localparam",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenvarDecl {
    pub ty: DataTy,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SpecparamDecl {
    pub ty: DataTy,
    pub decls: DeclsRange,
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn alloc_declaration<'ast, Ast>(
        &mut self,
        declaration: impl Into<Declaration>,
        ast: Ast,
    ) -> DeclarationId
    where
        Ast: syntax::ast::AstNode<'ast>,
    {
        let source = self.source_id(ast.syntax());
        let id = {
            let (declarations, sources) = self.declarations();
            crate::alloc_with_source_entry(declarations, sources, declaration, source)
        };
        self.record_body_declaration(id);
        id
    }

    pub(crate) fn finish_declaration_decls(&mut self, id: DeclarationId, decls: DeclsRange) {
        match &mut self.declarations().0[id] {
            Declaration::DataDecl(declaration) => declaration.decls = decls,
            Declaration::NetDecl(declaration) => declaration.decls = decls,
            Declaration::ParamDecl(declaration) => declaration.decls = decls,
            Declaration::GenvarDecl(declaration) => declaration.decls = decls,
            Declaration::SpecparamDecl(declaration) => declaration.decls = decls,
        }
    }

    pub(crate) fn lower_data_decl(&mut self, data_decl: ast::DataDeclaration) -> DeclarationId {
        let mut const_kw = false;
        let mut var_kw = false;
        data_decl.modifiers().children().for_each(|tok| match tok.kind() {
            TokenKind::CONST_KEYWORD => const_kw = true,
            TokenKind::VAR_KEYWORD => var_kw = true,
            TokenKind::UNKNOWN => {}
            _ => {}
        });

        let ty = self.lower_data_ty(data_decl.type_());

        let parent = self.alloc_declaration(
            DataDecl { ty, const_kw, var_kw, decls: empty_decls_range() },
            data_decl,
        );
        let decls = self.lower_declarators(data_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_net_decl(&mut self, net_decl: ast::NetDeclaration) -> DeclarationId {
        let net_kind = lower_net_kind(net_decl.net_type());
        let ty = self.lower_data_ty(net_decl.type_());
        let delay = net_decl.delay().and_then(|delay| {
            use crate::expr::timing_control::TimingControl::*;
            match self.lower_timing_control(delay) {
                DelayControl(delay) => Some(delay),
                _ => None,
            }
        });

        let strength = net_decl.strength().and_then(|strength| {
            use ast::NetStrength::*;
            match strength {
                PullStrength(strength) => {
                    strength.strength().and_then(lower_strength).map(NetStrength::Pull)
                }
                DriveStrength(strength) => Some(NetStrength::Drive(lower_drive_strength(strength))),
                ChargeStrength(strength) => {
                    strength.strength().and_then(lower_strength).map(NetStrength::Charge)
                }
            }
        });

        let parent = self.alloc_declaration(
            NetDecl { ty, net_kind, delay, strength, decls: empty_decls_range() },
            net_decl,
        );
        let decls = self.lower_declarators(net_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_port_decl_as_data_decl(
        &mut self,
        port_decl: ast::PortDeclaration,
    ) -> Option<DeclarationId> {
        use ast::PortHeader::*;
        let ty = match port_decl.header() {
            VariablePortHeader(header) => self.lower_data_ty(header.data_type()),
            NetPortHeader(header) => self.lower_data_ty(header.data_type()),
            InterfacePortHeader(_) => return None,
        };

        let parent = self.alloc_declaration(
            DataDecl { ty, const_kw: false, var_kw: false, decls: empty_decls_range() },
            port_decl,
        );
        let decls = self.lower_declarators(port_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        Some(parent)
    }

    pub(crate) fn lower_param_decl_base(
        &mut self,
        param_decl: ast::ParameterDeclarationBase,
    ) -> DeclarationId {
        self.lower_param_decl_base_with_context(param_decl, None, false, false)
    }

    pub(crate) fn lower_param_decl_base_with_context(
        &mut self,
        param_decl: ast::ParameterDeclarationBase,
        inherited_kind: Option<ParamDeclKind>,
        force_local: bool,
        is_port: bool,
    ) -> DeclarationId {
        use ast::ParameterDeclarationBase::*;
        match param_decl {
            ParameterDeclaration(param_decl) => {
                self.lower_param_decl(param_decl, inherited_kind, force_local, is_port)
            }
            TypeParameterDeclaration(type_param_decl) => {
                self.lower_type_param_decl(type_param_decl, inherited_kind, force_local, is_port)
            }
        }
    }

    fn lower_type_param_decl(
        &mut self,
        type_param_decl: ast::TypeParameterDeclaration,
        inherited_kind: Option<ParamDeclKind>,
        force_local: bool,
        is_port: bool,
    ) -> DeclarationId {
        let kind = lower_param_decl_kind(
            type_param_decl.keyword().map(|keyword| keyword.kind()),
            inherited_kind,
            force_local,
        );
        let decls = empty_decls_range();
        let ty = DataTy::Builtin(BuiltinDataTyId::new(BuiltinDataTy::default()));

        self.alloc_declaration(ParamDecl { ty, kind, is_port, decls }, type_param_decl)
    }

    fn lower_param_decl(
        &mut self,
        param_decl: ast::ParameterDeclaration,
        inherited_kind: Option<ParamDeclKind>,
        force_local: bool,
        is_port: bool,
    ) -> DeclarationId {
        let kind = lower_param_decl_kind(
            param_decl.keyword().map(|keyword| keyword.kind()),
            inherited_kind,
            force_local,
        );
        let ty = self.lower_data_ty(param_decl.type_());

        let parent = self.alloc_declaration(
            ParamDecl { ty, kind, is_port, decls: empty_decls_range() },
            param_decl,
        );
        let decls = self.lower_declarators(param_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_genvar_decl(
        &mut self,
        genvar_decl: ast::GenvarDeclaration,
    ) -> DeclarationId {
        let ty = DataTy::Builtin(BuiltinDataTyId::new(BuiltinDataTy::Int {
            kind: IntKind::Integer,
            signing: true,
        }));
        let parent =
            self.alloc_declaration(GenvarDecl { ty, decls: empty_decls_range() }, genvar_decl);
        let decls = self.lower_identifier_names(genvar_decl.identifiers(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_specparam_decl(
        &mut self,
        specparam_decl: ast::SpecparamDeclaration,
    ) -> DeclarationId {
        let ty = self.lower_implicit_data_ty(specparam_decl.type_());
        let parent = self
            .alloc_declaration(SpecparamDecl { ty, decls: empty_decls_range() }, specparam_decl);
        let decls = self.lower_specparam_declarators(specparam_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }
}

fn lower_param_decl_kind(
    keyword: Option<TokenKind>,
    inherited_kind: Option<ParamDeclKind>,
    force_local: bool,
) -> ParamDeclKind {
    if force_local {
        return ParamDeclKind::LocalParam;
    }

    match keyword {
        Some(TokenKind::LOCAL_PARAM_KEYWORD) => ParamDeclKind::LocalParam,
        Some(TokenKind::PARAMETER_KEYWORD) => ParamDeclKind::Parameter,
        _ => inherited_kind.unwrap_or(ParamDeclKind::Parameter),
    }
}
