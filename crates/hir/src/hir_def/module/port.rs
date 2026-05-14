use itertools::Either;
use la_arena::{Arena, Idx, IdxRange};
use syntax::{
    SyntaxToken, TokenKind,
    ast::{self, AstNode, PortExpression},
    ptr::SyntaxNodePtr,
};
use utils::get::{Get, GetRef};

use crate::{
    define_src, define_src_with_name,
    hir_def::{
        HirData, Ident, alloc_idx_and_src,
        expr::{
            LowerExpr, Selector,
            data_ty::{BuiltinDataTy, DataTy},
            declarator::{DeclsRange, LowerDecl},
        },
        lower_ident_opt,
        module::LowerModuleCtx,
        ty::{NetType, lower_net_kind},
    },
    source_map::SourceMap,
};

// structure:
//
// param ports:
// module name #(param_decls) (port_list {ansi, nonansi, wildcard})
//
// non-ansi ports:
// module name(non_ansi_port_list)
//   port_decl
//   data_decl
//
// ansi ports:
// module name(ansi_ports)

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PortDecl {
    pub header: PortHeader,
    pub decls: DeclsRange,
    pub name: Option<Ident>,
}

pub type PortDeclId = Idx<PortDecl>;

define_src!(PortDeclSrc(ast::ImplicitAnsiPort, ast::ExplicitAnsiPort, ast::PortDeclaration));

impl PortDeclSrc {
    pub fn ptr(&self) -> SyntaxNodePtr {
        match self {
            PortDeclSrc::ImplicitAnsiPort(ptr)
            | PortDeclSrc::ExplicitAnsiPort(ptr)
            | PortDeclSrc::PortDeclaration(ptr) => *ptr,
        }
    }
}

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
    pub fn dir(&self) -> Option<PortDirection> {
        match self {
            PortHeader::Var { dir, .. } | PortHeader::Net { dir, .. } => Some(*dir),
        }
    }

    pub fn ty(&self) -> DataTy {
        match self {
            PortHeader::Var { ty, .. } => *ty,
            PortHeader::Net { net_ty: NetType { ty, .. }, .. } => *ty,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Ports {
    NonAnsi { ports: Arena<NonAnsiPort>, refs: Arena<PortRef>, decls: Arena<PortDecl> },
    Ansi(Arena<PortDecl>),
}

pub type Port = Either<NonAnsiPort, PortDecl>;

impl Default for Ports {
    fn default() -> Self {
        Ports::Ansi(Arena::default())
    }
}

impl Ports {
    pub(crate) fn shrink_to_fit(&mut self) {
        match self {
            Ports::NonAnsi { ports, refs, decls } => {
                ports.shrink_to_fit();
                refs.shrink_to_fit();
                decls.shrink_to_fit();
            }
            Ports::Ansi(ports) => ports.shrink_to_fit(),
        }
    }
}

impl GetRef<PortDeclId> for Ports {
    type Output = PortDecl;

    fn get(&self, index: PortDeclId) -> &Self::Output {
        match self {
            Ports::NonAnsi { decls, .. } => &decls[index],
            Ports::Ansi(decls) => &decls[index],
        }
    }
}

impl GetRef<NonAnsiPortId> for Ports {
    type Output = NonAnsiPort;

    fn get(&self, index: NonAnsiPortId) -> &Self::Output {
        match self {
            Ports::NonAnsi { ports, .. } => &ports[index],
            Ports::Ansi(_) => unreachable!(),
        }
    }
}

impl GetRef<PortRefId> for Ports {
    type Output = PortRef;

    fn get(&self, index: PortRefId) -> &Self::Output {
        match self {
            Ports::NonAnsi { refs, .. } => &refs[index],
            Ports::Ansi(_) => unreachable!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NonAnsiPort {
    pub label: Option<Ident>,            // outside
    pub refs: Option<IdxRange<PortRef>>, // inside
}

pub type NonAnsiPortId = Idx<NonAnsiPort>;

define_src_with_name!(NonAnsiPortSrc(ast::NonAnsiPort));

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortRef {
    pub ident: Option<Ident>,
    pub select: Option<Selector>,
}

pub type PortRefId = Idx<PortRef>;

define_src_with_name!(PortRefSrc(ast::PortReference));

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PortSrcs {
    NonAnsi {
        ports: SourceMap<NonAnsiPortSrc, NonAnsiPort>,
        refs: SourceMap<PortRefSrc, PortRef>,
        decls: SourceMap<PortDeclSrc, PortDecl>,
        port_list_src: Option<PortListSrc>,
    },
    Ansi {
        decls: SourceMap<PortDeclSrc, PortDecl>,
        port_list_src: Option<PortListSrc>,
    },
}

impl PortSrcs {
    pub fn port_list_src(&self) -> Option<&PortListSrc> {
        match self {
            PortSrcs::NonAnsi { port_list_src, .. } | PortSrcs::Ansi { port_list_src, .. } => {
                port_list_src.as_ref()
            }
        }
    }
}

define_src!(PortListSrc(ast::NonAnsiPortList, ast::AnsiPortList, ast::WildcardPortList));

impl Default for PortSrcs {
    fn default() -> Self {
        PortSrcs::Ansi { decls: SourceMap::default(), port_list_src: None }
    }
}

impl Get<NonAnsiPortId> for PortSrcs {
    type Output = NonAnsiPortSrc;

    fn get(&self, port_id: NonAnsiPortId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get(port_id),
            PortSrcs::Ansi { .. } => unreachable!(),
        }
    }
}

impl Get<NonAnsiPortSrc> for PortSrcs {
    type Output = NonAnsiPortId;

    fn get(&self, src: NonAnsiPortSrc) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get(src),
            PortSrcs::Ansi { .. } => unreachable!(),
        }
    }
}

impl Get<PortRefId> for PortSrcs {
    type Output = PortRefSrc;

    fn get(&self, port_ref_id: PortRefId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get(port_ref_id),
            PortSrcs::Ansi { .. } => unreachable!(),
        }
    }
}

impl Get<PortRefSrc> for PortSrcs {
    type Output = PortRefId;

    fn get(&self, src: PortRefSrc) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get(src),
            PortSrcs::Ansi { .. } => unreachable!(),
        }
    }
}

impl Get<PortDeclId> for PortSrcs {
    type Output = PortDeclSrc;

    fn get(&self, port_id: PortDeclId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { decls, .. } => decls.get(port_id),
            PortSrcs::Ansi { decls, .. } => decls.get(port_id),
        }
    }
}

impl Get<PortDeclSrc> for PortSrcs {
    type Output = PortDeclId;

    fn get(&self, src: PortDeclSrc) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { decls, .. } => decls.get(src),
            PortSrcs::Ansi { decls, .. } => decls.get(src),
        }
    }
}

impl PortSrcs {
    pub fn shrink_to_fit(&mut self) {
        match self {
            PortSrcs::NonAnsi { ports, refs, decls, .. } => {
                ports.shrink_to_fit();
                refs.shrink_to_fit();
                decls.shrink_to_fit();
            }
            PortSrcs::Ansi { decls, .. } => decls.shrink_to_fit(),
        }
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_ansi_ports(&mut self, port_list: ast::AnsiPortList) {
        let mut ports = Arena::default();
        let mut decls = SourceMap::default();

        let mut header = None;
        for port in port_list.ports().children() {
            use ast::Member::*;
            match port {
                ImplicitAnsiPort(port) => {
                    header = Some(self.lower_port_header(port.header(), header));

                    let parent = ports.nxt_idx().into();
                    let decl_id = self.decl_ctx().lower_declarator(port.declarator(), parent);
                    let end = self.module.decls.nxt_idx();

                    alloc_idx_and_src! {
                        PortDecl {
                            header: header.unwrap(),
                            decls: IdxRange::new(decl_id..end),
                            name: None,
                        } => ports,
                        port => decls,
                    };
                }
                ExplicitAnsiPort(port) => {
                    header = Some(self.lower_explicit_ansi_header(port.direction(), header));
                    if let Some(expr) = port.expr() {
                        self.expr_ctx().lower_expr(expr);
                    }

                    let idx = ports.alloc(PortDecl {
                        header: header.unwrap(),
                        decls: IdxRange::new(
                            self.module.decls.nxt_idx()..self.module.decls.nxt_idx(),
                        ),
                        name: lower_ident_opt(port.name()),
                    });
                    decls.insert(port.into(), idx);
                }
                _ => unreachable!(),
            };
            self.region_tree.handle_node(port.syntax());
        }

        self.region_tree.stage(port_list.close_paren());

        self.module.ports = Ports::Ansi(ports);
        self.module_source_map.port_srcs =
            PortSrcs::Ansi { decls, port_list_src: Some(PortListSrc::from(port_list)) };
    }

    pub(crate) fn lower_wildcard_ports(&mut self, port_list: ast::WildcardPortList) {
        self.region_tree.stage(port_list.close_paren());
        self.module.ports = Ports::Ansi(Arena::default());
        self.module_source_map.port_srcs = PortSrcs::Ansi {
            decls: SourceMap::default(),
            port_list_src: Some(PortListSrc::from(port_list)),
        };
    }

    pub(crate) fn lower_nonansi_port(&mut self, port_list: ast::NonAnsiPortList) {
        let mut ports = Arena::default();
        let mut refs = Arena::default();
        let mut port_srcs = SourceMap::default();
        let mut ref_srcs: SourceMap<PortRefSrc, PortRef> = SourceMap::default();

        for port in port_list.ports().children() {
            use ast::{NonAnsiPort::*, PortExpression::*};

            let start = refs.nxt_idx();
            let mut lower_port_exprs = |exprs: Option<PortExpression>| {
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

                match exprs? {
                    PortConcatenation(concat) => {
                        concat.references().children().for_each(lower_port_ref);
                        Some(IdxRange::new(start..refs.nxt_idx()))
                    }
                    PortReference(port_ref) => {
                        lower_port_ref(port_ref);
                        Some(IdxRange::new(start..refs.nxt_idx()))
                    }
                }
            };

            let (hir_port, src_name) = match port {
                ExplicitNonAnsiPort(port) => (
                    NonAnsiPort {
                        label: lower_ident_opt(port.name()),
                        refs: lower_port_exprs(port.expr()),
                    },
                    None,
                ),
                ImplicitNonAnsiPort(port) => {
                    let sub_refs = lower_port_exprs(Some(port.expr()));
                    debug_assert!(sub_refs.as_ref().is_none_or(|refs| refs.len() == 1));

                    let port_ref_id = sub_refs.as_ref().unwrap().start();
                    let label = refs[port_ref_id].ident.clone();
                    let src_name = ref_srcs.get(port_ref_id).name;
                    (NonAnsiPort { label, refs: sub_refs }, src_name)
                }
                EmptyNonAnsiPort(_) => (NonAnsiPort { label: None, refs: None }, None),
            };

            self.region_tree.handle_node(port.syntax());
            let port_id = alloc_idx_and_src! {
                hir_port => ports,
                port => port_srcs,
            };
            if src_name.is_some() {
                port_srcs
                    .insert(NonAnsiPortSrc { name: src_name, ..port_srcs.get(port_id) }, port_id);
            }
        }

        self.region_tree.stage(port_list.close_paren());

        self.module.ports = Ports::NonAnsi { ports, refs, decls: Arena::default() };
        self.module_source_map.port_srcs = PortSrcs::NonAnsi {
            ports: port_srcs,
            refs: ref_srcs,
            decls: SourceMap::default(),
            port_list_src: Some(PortListSrc::from(port_list)),
        };
    }

    pub(crate) fn lower_port_decl(&mut self, decl: ast::PortDeclaration) -> PortDeclId {
        let header = self.lower_port_header(decl.header(), None);

        let parent = match &self.module.ports {
            Ports::NonAnsi { decls, .. } | Ports::Ansi(decls) => decls.nxt_idx().into(),
        };

        let decls = self.decl_ctx().lower_declarators(decl.declarators(), parent);

        match (&mut self.module.ports, &mut self.module_source_map.port_srcs) {
            (Ports::NonAnsi { decls: port_decls, .. }, PortSrcs::NonAnsi { decls: srcs, .. })
            | (Ports::Ansi(port_decls), PortSrcs::Ansi { decls: srcs, .. }) => {
                alloc_idx_and_src! {
                    PortDecl { header, decls, name: None } => port_decls,
                    decl => srcs,
                }
            }
            _ => unreachable!(),
        }
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
        let prev_header = prev_header.unwrap_or_else(|| self.default_port_header());

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
                    None => {
                        if matches!(dir, PortDirection::Input | PortDirection::Inout)
                            || (matches!(dir, PortDirection::Output)
                                && matches!(ast_ty, ast::DataType::ImplicitType(_)))
                        {
                            PortHeader::Net { dir, net_ty: NetType { kind: default_net_kind, ty } }
                        } else {
                            PortHeader::Var { dir, var_kw: false, ty }
                        }
                    }
                }
            }
            InterfacePortHeader(_header) => prev_header,
        }
    }

    fn lower_explicit_ansi_header(
        &mut self,
        direction: Option<SyntaxToken>,
        prev_header: Option<PortHeader>,
    ) -> PortHeader {
        let dir = Self::lower_dir(direction);
        let prev_header = prev_header.unwrap_or_else(|| self.default_port_header());
        let Some(dir) = dir else {
            return prev_header;
        };

        match prev_header {
            PortHeader::Var { var_kw, ty, .. } => PortHeader::Var { dir, var_kw, ty },
            PortHeader::Net { net_ty, .. } => PortHeader::Net { dir, net_ty },
        }
    }

    fn default_port_header(&mut self) -> PortHeader {
        let default_data_ty = DataTy::Builtin(self.db.intern_ty(BuiltinDataTy::default()));
        let default_net_kind = self.default_net_type.unwrap();
        PortHeader::Net {
            dir: PortDirection::default(),
            net_ty: NetType { kind: default_net_kind, ty: default_data_ty },
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
