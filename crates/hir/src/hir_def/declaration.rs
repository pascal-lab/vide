use la_arena::Idx;
use syntax::{TokenKind, ast, ptr::SyntaxNodePtr};
use utils::define_enum_deriving_from;

use super::expr::{
    data_ty::{BuiltinDataTy, DataTy, IntKind},
    declarator::{DeclsRange, empty_decls_range},
    timing_control::DelayControl,
};
use crate::{
    hir_def::{
        alloc_with_source,
        lower::{LoweringCtx, LoweringStore},
        ty::{
            DriveStrength, NetKind, Strength, lower_drive_strength, lower_net_kind, lower_strength,
        },
    },
    source_map::{
        AstId, AstKind, FromSourceAst, IsSrc, SourceAst, ToAstNode, exact_ast_node_from_ptr,
    },
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DataDeclarationAst;

impl AstKind for DataDeclarationAst {
    type Node<'a> = ast::DataDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NetDeclarationAst;

impl AstKind for NetDeclarationAst {
    type Node<'a> = ast::NetDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DeclarationPortDeclarationAst;

impl AstKind for DeclarationPortDeclarationAst {
    type Node<'a> = ast::PortDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ParameterDeclarationAst;

impl AstKind for ParameterDeclarationAst {
    type Node<'a> = ast::ParameterDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct TypeParameterDeclarationAst;

impl AstKind for TypeParameterDeclarationAst {
    type Node<'a> = ast::TypeParameterDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LocalVariableDeclarationAst;

impl AstKind for LocalVariableDeclarationAst {
    type Node<'a> = ast::LocalVariableDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct GenvarDeclarationAst;

impl AstKind for GenvarDeclarationAst {
    type Node<'a> = ast::GenvarDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SpecparamDeclarationAst;

impl AstKind for SpecparamDeclarationAst {
    type Node<'a> = ast::SpecparamDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum DeclarationSrc {
    DataDeclaration(AstId<DataDeclarationAst>),
    NetDeclaration(AstId<NetDeclarationAst>),
    PortDeclaration(AstId<DeclarationPortDeclarationAst>),
    ParameterDeclaration(AstId<ParameterDeclarationAst>),
    TypeParameterDeclaration(AstId<TypeParameterDeclarationAst>),
    LocalVariableDeclaration(AstId<LocalVariableDeclarationAst>),
    GenvarDeclaration(AstId<GenvarDeclarationAst>),
    SpecparamDeclaration(AstId<SpecparamDeclarationAst>),
}

impl DeclarationSrc {
    pub fn ptr(&self) -> SyntaxNodePtr {
        match self {
            DeclarationSrc::DataDeclaration(src) => src.ptr(),
            DeclarationSrc::NetDeclaration(src) => src.ptr(),
            DeclarationSrc::PortDeclaration(src) => src.ptr(),
            DeclarationSrc::ParameterDeclaration(src) => src.ptr(),
            DeclarationSrc::TypeParameterDeclaration(src) => src.ptr(),
            DeclarationSrc::LocalVariableDeclaration(src) => src.ptr(),
            DeclarationSrc::GenvarDeclaration(src) => src.ptr(),
            DeclarationSrc::SpecparamDeclaration(src) => src.ptr(),
        }
    }
}

impl IsSrc for DeclarationSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.ptr().kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.ptr().range()
    }
}

impl<'a> ToAstNode<'a, ast::DataDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::DataDeclaration<'a>> {
        let DeclarationSrc::DataDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::NetDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::NetDeclaration<'a>> {
        let DeclarationSrc::NetDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::PortDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PortDeclaration<'a>> {
        let DeclarationSrc::PortDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::ParameterDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ParameterDeclaration<'a>> {
        let DeclarationSrc::ParameterDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::TypeParameterDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::TypeParameterDeclaration<'a>> {
        let DeclarationSrc::TypeParameterDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::LocalVariableDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::LocalVariableDeclaration<'a>> {
        let DeclarationSrc::LocalVariableDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::GenvarDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::GenvarDeclaration<'a>> {
        let DeclarationSrc::GenvarDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::SpecparamDeclaration<'a>> for DeclarationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::SpecparamDeclaration<'a>> {
        let DeclarationSrc::SpecparamDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> FromSourceAst<'a, ast::DataDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::DataDeclaration<'a>>) -> Self {
        Self::DataDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::NetDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::NetDeclaration<'a>>) -> Self {
        Self::NetDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::PortDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::PortDeclaration<'a>>) -> Self {
        Self::PortDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::ParameterDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::ParameterDeclaration<'a>>) -> Self {
        Self::ParameterDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::TypeParameterDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::TypeParameterDeclaration<'a>>) -> Self {
        Self::TypeParameterDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::LocalVariableDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::LocalVariableDeclaration<'a>>) -> Self {
        Self::LocalVariableDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::GenvarDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::GenvarDeclaration<'a>>) -> Self {
        Self::GenvarDeclaration(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::SpecparamDeclaration<'a>> for DeclarationSrc {
    fn from_source_ast(node: SourceAst<ast::SpecparamDeclaration<'a>>) -> Self {
        Self::SpecparamDeclaration(AstId::from_source_ast(node))
    }
}

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
            Declaration::DataDecl(data_decl) => data_decl.ty,
            Declaration::NetDecl(net_decl) => net_decl.ty,
            Declaration::ParamDecl(param_decl) => param_decl.ty,
            Declaration::GenvarDecl(genvar_decl) => genvar_decl.ty,
            Declaration::SpecparamDecl(specparam_decl) => specparam_decl.ty,
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

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
    pub(crate) fn alloc_declaration<'ast, Ast>(
        &mut self,
        declaration: impl Into<Declaration>,
        ast: Ast,
    ) -> DeclarationId
    where
        Ast: syntax::ast::AstNode<'ast>,
        DeclarationSrc: FromSourceAst<'ast, Ast>,
    {
        let file_id = self.file_id;
        let (declarations, sources) = self.declarations();
        alloc_with_source(file_id, declarations, sources, declaration, ast)
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
            use crate::hir_def::expr::timing_control::TimingControl::*;
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
        let ty = DataTy::Builtin(
            self.db.intern_ty(crate::hir_def::expr::data_ty::BuiltinDataTy::default()),
        );

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
        let ty = DataTy::Builtin(
            self.db.intern_ty(BuiltinDataTy::Int { kind: IntKind::Integer, signing: true }),
        );
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
