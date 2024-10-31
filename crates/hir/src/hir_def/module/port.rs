use std::ops::Index;

use itertools::Either;
use la_arena::{Arena, Idx, IdxRange, RawIdx};
use syntax::{SyntaxToken, TokenKind, ast};
use utils::get::Get;

use crate::{
    alloc_idx_and_src, define_src,
    hir_def::{
        Ident, arena_nxt_idx,
        expr::{
            LowerExpr, Selector,
            data_ty::{BuiltinDataTy, DataTy},
            declarator::{DeclIdRange, Declarator, LowerDecl},
        },
        lower_ident_opt,
        module::LowerModuleCtx,
        ty::{NetType, lower_net_kind},
    },
    source_map::SourceMap,
};

// module IDENT (port_list);
// port_decl;
// declaration?;
// endmodule

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PortDecl {
    pub header: PortHeader,
    pub decls: DeclIdRange,
}

pub type PortDeclId = Idx<PortDecl>;

define_src!(PortDeclSrc(ast::ImplicitAnsiPort, ast::PortDeclaration));

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortDirection {
    Input,
    Output,
    Ref,
    #[default]
    Inout,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortHeader {
    Var { dir: PortDirection, var_kw: bool, ty: DataTy },
    Net { dir: PortDirection, net_ty: NetType },
}

impl PortHeader {
    pub(crate) fn dir(&self) -> Option<PortDirection> {
        match self {
            PortHeader::Var { dir, .. } | PortHeader::Net { dir, .. } => Some(*dir),
        }
    }

    pub(crate) fn ty(&self) -> DataTy {
        match self {
            PortHeader::Var { ty, .. } => *ty,
            PortHeader::Net { net_ty: NetType { ty, .. }, .. } => *ty,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Ports {
    NonAnsi { ports: Arena<NonAnsiPort>, refs: Arena<PortRef> },
    Ansi(Arena<AnsiPort>),
}

pub type PortId = Either<NonAnsiPortId, AnsiPortId>;

impl Default for Ports {
    fn default() -> Self {
        Ports::Ansi(Arena::default())
    }
}

impl Ports {
    pub(crate) fn shrink_to_fit(&mut self) {
        match self {
            Ports::NonAnsi { ports, refs } => {
                ports.shrink_to_fit();
                refs.shrink_to_fit();
            }
            Ports::Ansi(ports) => ports.shrink_to_fit(),
        }
    }
}

impl Index<AnsiPortId> for Ports {
    type Output = AnsiPort;

    fn index(&self, index: AnsiPortId) -> &Self::Output {
        match self {
            Ports::NonAnsi { .. } => unreachable!(),
            Ports::Ansi(ports) => &ports[index],
        }
    }
}

impl Index<NonAnsiPortId> for Ports {
    type Output = NonAnsiPort;

    fn index(&self, index: NonAnsiPortId) -> &Self::Output {
        match self {
            Ports::NonAnsi { ports, .. } => &ports[index],
            Ports::Ansi(_) => unreachable!(),
        }
    }
}

impl Index<PortRefId> for Ports {
    type Output = PortRef;

    fn index(&self, index: PortRefId) -> &Self::Output {
        match self {
            Ports::NonAnsi { refs, .. } => &refs[index],
            Ports::Ansi(_) => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NonAnsiPort {
    pub label: Option<Ident>,
    pub refs: Option<IdxRange<PortRef>>,
}

pub type NonAnsiPortId = Idx<NonAnsiPort>;

define_src!(NonAnsiPortSrc(ast::NonAnsiPort));

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortRef {
    pub ident: Option<Ident>,
    pub select: Option<Selector>,
}

pub type PortRefId = Idx<PortRef>;

define_src!(PortRefSrc(ast::PortReference));

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AnsiPort {
    pub decl: PortDeclId,
}

pub type AnsiPortId = Idx<AnsiPort>;

define_src!(AnsiPortSrc(ast::ImplicitAnsiPort));

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PortSrcs {
    NonAnsi { ports: SourceMap<NonAnsiPortSrc, NonAnsiPort>, refs: SourceMap<PortRefSrc, PortRef> },
    Ansi(SourceMap<AnsiPortSrc, AnsiPort>),
}

impl Default for PortSrcs {
    fn default() -> Self {
        PortSrcs::Ansi(SourceMap::default())
    }
}

impl Get<NonAnsiPortId> for PortSrcs {
    type Output = NonAnsiPortSrc;

    fn get_opt(&self, port_id: NonAnsiPortId) -> Option<Self::Output> {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get_opt(port_id),
            PortSrcs::Ansi(_) => None,
        }
    }
}

impl Get<NonAnsiPortSrc> for PortSrcs {
    type Output = NonAnsiPortId;

    fn get_opt(&self, src: NonAnsiPortSrc) -> Option<Self::Output> {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get_opt(src),
            PortSrcs::Ansi(_) => None,
        }
    }
}

impl Get<PortRefId> for PortSrcs {
    type Output = PortRefSrc;

    fn get_opt(&self, port_ref_id: PortRefId) -> Option<Self::Output> {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get_opt(port_ref_id),
            PortSrcs::Ansi(_) => None,
        }
    }
}

impl Get<PortRefSrc> for PortSrcs {
    type Output = PortRefId;

    fn get_opt(&self, src: PortRefSrc) -> Option<Self::Output> {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get_opt(src),
            PortSrcs::Ansi(_) => None,
        }
    }
}

impl Get<AnsiPortId> for PortSrcs {
    type Output = AnsiPortSrc;

    fn get_opt(&self, port_id: AnsiPortId) -> Option<Self::Output> {
        match self {
            PortSrcs::Ansi(ports) => ports.get_opt(port_id),
            PortSrcs::NonAnsi { .. } => None,
        }
    }
}

impl Get<AnsiPortSrc> for PortSrcs {
    type Output = AnsiPortId;

    fn get_opt(&self, src: AnsiPortSrc) -> Option<Self::Output> {
        match self {
            PortSrcs::Ansi(ports) => ports.get_opt(src),
            PortSrcs::NonAnsi { .. } => None,
        }
    }
}

impl PortSrcs {
    pub fn shrink_to_fit(&mut self) {
        match self {
            PortSrcs::NonAnsi { ports, refs } => {
                ports.shrink_to_fit();
                refs.shrink_to_fit();
            }
            PortSrcs::Ansi(ports) => ports.shrink_to_fit(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParamPort {
    pub ty: DataTy,
    pub decl: IdxRange<Declarator>,
}

pub type ParamPortId = Idx<ParamPort>;

define_src!(ParamPortSrc(ast::ParameterDeclaration));

impl ParamPort {
    pub(crate) fn new(ty: DataTy, start: usize, end: usize) -> Self {
        let start = Idx::from_raw(RawIdx::from(start as u32));
        let end = Idx::from_raw(RawIdx::from(end as u32));
        Self { ty, decl: IdxRange::new(start..end) }
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_param_ports(&mut self, param_ports: ast::ParameterPortList) {
        for decl in param_ports.declarations().children() {
            use ast::ParameterDeclarationBase::*;
            match decl {
                ParameterDeclaration(param) => {
                    let ty = self.expr_ctx().lower_data_ty(param.type_());

                    let next_param_port_idx = arena_nxt_idx(&self.module.params).into();
                    let start = arena_nxt_idx(&self.module.decls);
                    param.declarators().children().for_each(|decl| {
                        self.decl_ctx().lower_declarator(decl, next_param_port_idx);
                    });
                    let end = arena_nxt_idx(&self.module.decls);

                    alloc_idx_and_src! {
                        ParamPort { ty, decl: IdxRange::new(start..end) } => self.module.params,
                        param => self.module_source_map.param_srcs,
                    };
                }
                TypeParameterDeclaration(_) => unimplemented!(),
            }
        }
    }

    pub(crate) fn lower_ansi_ports(&mut self, port_list: ast::AnsiPortList) {
        let mut ports = Arena::default();
        let mut srcs = SourceMap::default();

        let mut header = None;
        for port in port_list.ports().children() {
            use ast::Member::*;
            match port {
                ImplicitAnsiPort(port) => {
                    header = Some(self.lower_port_header(port.header(), header));

                    let next_ansi_port_idx = arena_nxt_idx(&ports).into();
                    let decl_id =
                        self.decl_ctx().lower_declarator(port.declarator(), next_ansi_port_idx);
                    let end = arena_nxt_idx(&self.module.decls);

                    let port_decl_idx = alloc_idx_and_src! {
                        PortDecl {
                            header: header.unwrap(),
                            decls: IdxRange::new(decl_id..end)
                        } => self.module.port_decls,
                        port => self.module_source_map.prot_decl_srcs,
                    };

                    alloc_idx_and_src! {
                        AnsiPort { decl: port_decl_idx } => ports,
                        port => srcs,
                    };
                }
                ExplicitAnsiPort(_port) => unimplemented!(),
                _ => unreachable!(),
            };
        }

        self.module.ports = Ports::Ansi(ports);
        self.module_source_map.port_srcs = PortSrcs::Ansi(srcs);
    }

    pub(crate) fn lower_nonansi_port(&mut self, port_list: ast::NonAnsiPortList) {
        let mut ports = Arena::default();
        let mut refs = Arena::default();
        let mut port_srcs = SourceMap::default();
        let mut ref_srcs = SourceMap::default();

        for port in port_list.ports().children() {
            use ast::{NonAnsiPort::*, PortExpression::*};
            let hir_port = {
                let (label, exprs) = match port {
                    ExplicitNonAnsiPort(port) => (lower_ident_opt(port.name()), port.expr()),
                    ImplicitNonAnsiPort(port) => (None, Some(port.expr())),
                    EmptyNonAnsiPort(_) => (None, None),
                };

                let start = arena_nxt_idx(&refs);

                let mut lower_port_ref = |port_ref: ast::PortReference| {
                    let ident = lower_ident_opt(port_ref.name());
                    let select = port_ref
                        .select()
                        .and_then(|sel| sel.selector())
                        .map(|sel| self.expr_ctx().lower_selector(sel));
                    alloc_idx_and_src! {
                        PortRef { ident, select } => refs,
                        port_ref => ref_srcs,
                    };
                };

                match exprs {
                    Some(PortConcatenation(concat)) => {
                        concat.references().children().for_each(|port_ref| {
                            lower_port_ref(port_ref);
                        });
                        let end = arena_nxt_idx(&refs);
                        NonAnsiPort { label, refs: Some(IdxRange::new(start..end)) }
                    }
                    Some(PortReference(port_ref)) => {
                        lower_port_ref(port_ref);
                        let end = arena_nxt_idx(&refs);
                        NonAnsiPort { label, refs: Some(IdxRange::new(start..end)) }
                    }
                    None => NonAnsiPort { label, refs: None },
                }
            };

            alloc_idx_and_src! {
                hir_port => ports,
                port => port_srcs,
            };
        }

        self.module.ports = Ports::NonAnsi { ports, refs };
        self.module_source_map.port_srcs = PortSrcs::NonAnsi { ports: port_srcs, refs: ref_srcs };
    }

    pub(crate) fn lower_port_decl(&mut self, decl: ast::PortDeclaration) {
        let header = self.lower_port_header(decl.header(), None);

        let next_port_decl_idx = arena_nxt_idx(&self.module.port_decls).into();
        let start = arena_nxt_idx(&self.module.decls);
        decl.declarators().children().for_each(|decl| {
            self.decl_ctx().lower_declarator(decl, next_port_decl_idx);
        });
        let end = arena_nxt_idx(&self.module.decls);

        alloc_idx_and_src! {
            PortDecl { header, decls: IdxRange::new(start..end) } => self.module.port_decls,
            decl => self.module_source_map.prot_decl_srcs,
        };
    }

    // Port header may inherit properties from the previous port header, so we
    // need to keep track of the previous port header.
    fn lower_port_header(
        &mut self,
        header: ast::PortHeader,
        prev_header: Option<PortHeader>,
    ) -> PortHeader {
        let default_data_ty = DataTy::Builtin(self.db.intern_ty(BuiltinDataTy::default()));
        let default_net_kind = self.default_net_type.unwrap();
        let prev_header = prev_header.unwrap_or_else(|| PortHeader::Net {
            dir: PortDirection::default(),
            net_ty: NetType { kind: default_net_kind, ty: default_data_ty },
        });

        use ast::PortHeader::*;
        match header {
            VariablePortHeader(_) | NetPortHeader(_) => {
                // Extract information from the AST
                let (ast_dir, port_kind, ast_ty) = match &header {
                    VariablePortHeader(header) => {
                        let var_kw = header.var_keyword().map(|_| Either::Left(()));
                        (header.direction(), var_kw, header.data_type())
                    }
                    NetPortHeader(header) => (
                        header.direction(),
                        lower_net_kind(header.net_type()).map(Either::Right),
                        header.data_type(),
                    ),
                    _ => unreachable!(),
                };

                // Check if omitted
                let ty_omitted = DataTy::is_ast_missing(ast_ty);
                let all_omitted = ast_dir.is_none() && port_kind.is_none() && ty_omitted;

                // Generate the header
                let dir = Self::lower_dir(ast_dir).or(prev_header.dir()).unwrap();

                let ty = if !ty_omitted {
                    self.expr_ctx().lower_data_ty(ast_ty)
                } else if all_omitted {
                    prev_header.ty()
                } else {
                    default_data_ty
                };

                match port_kind {
                    Some(Either::Left(())) => PortHeader::Var { dir, var_kw: true, ty },
                    Some(Either::Right(kind)) => {
                        PortHeader::Net { dir, net_ty: NetType { kind, ty } }
                    }
                    None => match dir {
                        PortDirection::Input | PortDirection::Inout => {
                            PortHeader::Net { dir, net_ty: NetType { kind: default_net_kind, ty } }
                        }
                        PortDirection::Output
                            if matches!(ast_ty, ast::DataType::ImplicitType(_)) =>
                        {
                            PortHeader::Net { dir, net_ty: NetType { kind: default_net_kind, ty } }
                        }
                        _ => PortHeader::Var { dir, var_kw: false, ty },
                    },
                }
            }
            InterfacePortHeader(_header) => unimplemented!(),
        }
    }

    fn lower_dir(tok: Option<SyntaxToken>) -> Option<PortDirection> {
        tok.map(|tok| match tok.kind() {
            TokenKind::INPUT_KEYWORD => PortDirection::Input,
            TokenKind::OUTPUT_KEYWORD => PortDirection::Output,
            TokenKind::IN_OUT_KEYWORD => PortDirection::Inout,
            TokenKind::REF_KEYWORD => PortDirection::Ref,
            _ => unreachable!(),
        })
    }
}
