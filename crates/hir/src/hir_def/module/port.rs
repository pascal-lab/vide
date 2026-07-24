use itertools::Either;
use la_arena::{Arena, Idx, IdxRange};
use syntax::{
    SyntaxToken, TokenKind,
    ast::{self, AstNode, PortExpression},
    ptr::SyntaxNodePtr,
};
use utils::get::{Get, GetRef};

use crate::{
    hir_def::{
        Ident, alloc_with_optional_source_entry, alloc_with_source,
        expr::{
            Selector,
            data_ty::{BuiltinDataTy, DataTy},
            declarator::{DeclsRange, empty_decls_range},
        },
        lower_ident_opt,
        module::LowerModuleCtx,
        ty::{NetType, lower_net_kind},
    },
    source_map::{
        AstId, AstKind, FromSourceAst, IsSrc, NamedAstId, SourceAst, SourceMap, ToAstNode,
        exact_ast_node_from_ptr,
    },
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ImplicitAnsiPortAst;

impl AstKind for ImplicitAnsiPortAst {
    type Node<'a> = ast::ImplicitAnsiPort<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ExplicitAnsiPortAst;

impl AstKind for ExplicitAnsiPortAst {
    type Node<'a> = ast::ExplicitAnsiPort<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PortDeclarationAst;

impl AstKind for PortDeclarationAst {
    type Node<'a> = ast::PortDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortDeclSrc {
    ImplicitAnsiPort(AstId<ImplicitAnsiPortAst>),
    ExplicitAnsiPort(AstId<ExplicitAnsiPortAst>),
    PortDeclaration(AstId<PortDeclarationAst>),
}

impl PortDeclSrc {
    pub fn ptr(&self) -> SyntaxNodePtr {
        match self {
            PortDeclSrc::ImplicitAnsiPort(src) => src.ptr(),
            PortDeclSrc::ExplicitAnsiPort(src) => src.ptr(),
            PortDeclSrc::PortDeclaration(src) => src.ptr(),
        }
    }
}

impl IsSrc for PortDeclSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.ptr().kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.ptr().range()
    }
}

impl<'a> ToAstNode<'a, ast::ImplicitAnsiPort<'a>> for PortDeclSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ImplicitAnsiPort<'a>> {
        let PortDeclSrc::ImplicitAnsiPort(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::ExplicitAnsiPort<'a>> for PortDeclSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ExplicitAnsiPort<'a>> {
        let PortDeclSrc::ExplicitAnsiPort(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::PortDeclaration<'a>> for PortDeclSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PortDeclaration<'a>> {
        let PortDeclSrc::PortDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> FromSourceAst<'a, ast::ImplicitAnsiPort<'a>> for PortDeclSrc {
    fn from_source_ast(port: SourceAst<ast::ImplicitAnsiPort<'a>>) -> Self {
        Self::ImplicitAnsiPort(AstId::from_source_ast(port))
    }
}

impl<'a> FromSourceAst<'a, ast::ExplicitAnsiPort<'a>> for PortDeclSrc {
    fn from_source_ast(port: SourceAst<ast::ExplicitAnsiPort<'a>>) -> Self {
        Self::ExplicitAnsiPort(AstId::from_source_ast(port))
    }
}

impl<'a> FromSourceAst<'a, ast::PortDeclaration<'a>> for PortDeclSrc {
    fn from_source_ast(port: SourceAst<ast::PortDeclaration<'a>>) -> Self {
        Self::PortDeclaration(AstId::from_source_ast(port))
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
    pub fn dir(&self) -> PortDirection {
        match self {
            PortHeader::Var { dir, .. } | PortHeader::Net { dir, .. } => *dir,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NonAnsiPortAst;

impl AstKind for NonAnsiPortAst {
    type Node<'a> = ast::NonAnsiPort<'a>;
}

pub type NonAnsiPortSrc = NamedAstId<NonAnsiPortAst>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortRef {
    pub ident: Option<Ident>,
    pub select: Option<Selector>,
}

pub type PortRefId = Idx<PortRef>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PortReferenceAst;

impl AstKind for PortReferenceAst {
    type Node<'a> = ast::PortReference<'a>;
}

pub type PortRefSrc = NamedAstId<PortReferenceAst>;

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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NonAnsiPortListAst;

impl AstKind for NonAnsiPortListAst {
    type Node<'a> = ast::NonAnsiPortList<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct AnsiPortListAst;

impl AstKind for AnsiPortListAst {
    type Node<'a> = ast::AnsiPortList<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct WildcardPortListAst;

impl AstKind for WildcardPortListAst {
    type Node<'a> = ast::WildcardPortList<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortListSrc {
    NonAnsiPortList(AstId<NonAnsiPortListAst>),
    AnsiPortList(AstId<AnsiPortListAst>),
    WildcardPortList(AstId<WildcardPortListAst>),
}

impl IsSrc for PortListSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        match self {
            PortListSrc::NonAnsiPortList(src) => src.ptr().kind(),
            PortListSrc::AnsiPortList(src) => src.ptr().kind(),
            PortListSrc::WildcardPortList(src) => src.ptr().kind(),
        }
    }

    fn range(&self) -> utils::text_edit::TextRange {
        match self {
            PortListSrc::NonAnsiPortList(src) => src.ptr().range(),
            PortListSrc::AnsiPortList(src) => src.ptr().range(),
            PortListSrc::WildcardPortList(src) => src.ptr().range(),
        }
    }
}

impl<'a> ToAstNode<'a, ast::NonAnsiPortList<'a>> for PortListSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::NonAnsiPortList<'a>> {
        let PortListSrc::NonAnsiPortList(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::AnsiPortList<'a>> for PortListSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::AnsiPortList<'a>> {
        let PortListSrc::AnsiPortList(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::WildcardPortList<'a>> for PortListSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::WildcardPortList<'a>> {
        let PortListSrc::WildcardPortList(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> FromSourceAst<'a, ast::NonAnsiPortList<'a>> for PortListSrc {
    fn from_source_ast(list: SourceAst<ast::NonAnsiPortList<'a>>) -> Self {
        Self::NonAnsiPortList(AstId::from_source_ast(list))
    }
}

impl<'a> FromSourceAst<'a, ast::AnsiPortList<'a>> for PortListSrc {
    fn from_source_ast(list: SourceAst<ast::AnsiPortList<'a>>) -> Self {
        Self::AnsiPortList(AstId::from_source_ast(list))
    }
}

impl<'a> FromSourceAst<'a, ast::WildcardPortList<'a>> for PortListSrc {
    fn from_source_ast(list: SourceAst<ast::WildcardPortList<'a>>) -> Self {
        Self::WildcardPortList(AstId::from_source_ast(list))
    }
}

impl Default for PortSrcs {
    fn default() -> Self {
        PortSrcs::Ansi { decls: SourceMap::default(), port_list_src: None }
    }
}

impl Get<NonAnsiPortId> for PortSrcs {
    type Output = Option<NonAnsiPortSrc>;

    fn get(&self, port_id: NonAnsiPortId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get(port_id),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<NonAnsiPortSrc> for PortSrcs {
    type Output = Option<NonAnsiPortId>;

    fn get(&self, src: NonAnsiPortSrc) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get(src),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<PortRefId> for PortSrcs {
    type Output = Option<PortRefSrc>;

    fn get(&self, port_ref_id: PortRefId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get(port_ref_id),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<PortRefSrc> for PortSrcs {
    type Output = Option<PortRefId>;

    fn get(&self, src: PortRefSrc) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get(src),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<PortDeclId> for PortSrcs {
    type Output = Option<PortDeclSrc>;

    fn get(&self, port_id: PortDeclId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { decls, .. } => decls.get(port_id),
            PortSrcs::Ansi { decls, .. } => decls.get(port_id),
        }
    }
}

impl Get<PortDeclSrc> for PortSrcs {
    type Output = Option<PortDeclId>;

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
        let mut ports: Arena<PortDecl> = Arena::default();
        let mut decls: SourceMap<PortDeclSrc, PortDecl> = SourceMap::default();

        let mut header = None;
        for port in port_list.ports().children() {
            use ast::Member::*;
            match port {
                ImplicitAnsiPort(port) => {
                    header = Some(self.lower_port_header(port.header(), header));
                    let current_header = header.unwrap_or_else(|| self.default_port_header());
                    header = Some(current_header);
                    let parent = alloc_with_source(
                        self.file_id,
                        &mut ports,
                        &mut decls,
                        PortDecl { header: current_header, decls: empty_decls_range(), name: None },
                        port,
                    );
                    let decl_id = self.lower_declarator(port.declarator(), parent.into());
                    ports[parent].decls = IdxRange::new_inclusive(decl_id..=decl_id);
                }
                ExplicitAnsiPort(port) => {
                    header = Some(self.lower_explicit_ansi_header(port.direction(), header));
                    if let Some(expr) = port.expr() {
                        self.lower_expr(expr);
                    }

                    let current_header = header.unwrap_or_else(|| self.default_port_header());
                    header = Some(current_header);
                    alloc_with_source(
                        self.file_id,
                        &mut ports,
                        &mut decls,
                        PortDecl {
                            header: current_header,
                            decls: empty_decls_range(),
                            name: lower_ident_opt(port.name()),
                        },
                        port,
                    );
                }
                _ => continue,
            };
            self.region_tree.handle_node(port.syntax());
        }

        self.region_tree.stage(port_list.close_paren(), port_list.syntax());

        self.store.data.ports = Ports::Ansi(ports);
        self.store.sources.port_srcs = PortSrcs::Ansi {
            decls,
            port_list_src: Some(PortListSrc::AnsiPortList(AstId::from_ast(
                self.file_id,
                port_list,
            ))),
        };
    }

    pub(crate) fn lower_wildcard_ports(&mut self, port_list: ast::WildcardPortList) {
        self.region_tree.stage(port_list.close_paren(), port_list.syntax());
        self.store.data.ports = Ports::Ansi(Arena::default());
        self.store.sources.port_srcs = PortSrcs::Ansi {
            decls: SourceMap::default(),
            port_list_src: Some(PortListSrc::WildcardPortList(AstId::from_ast(
                self.file_id,
                port_list,
            ))),
        };
    }

    pub(crate) fn lower_nonansi_port(&mut self, port_list: ast::NonAnsiPortList) {
        let mut ports = Arena::default();
        let mut refs = Arena::default();
        let mut port_srcs: SourceMap<NonAnsiPortSrc, NonAnsiPort> = SourceMap::default();
        let mut ref_srcs: SourceMap<PortRefSrc, PortRef> = SourceMap::default();

        for port in port_list.ports().children() {
            use ast::{NonAnsiPort::*, PortExpression::*};

            let mut lower_port_exprs = |exprs: Option<PortExpression>| {
                let mut lower_port_ref = |port_ref: ast::PortReference| {
                    let ident = lower_ident_opt(port_ref.name());
                    let select = port_ref
                        .select()
                        .and_then(|sel| sel.selector())
                        .map(|sel| self.lower_selector(sel));
                    alloc_with_source(
                        self.file_id,
                        &mut refs,
                        &mut ref_srcs,
                        PortRef { ident, select },
                        port_ref,
                    )
                };

                match exprs? {
                    PortConcatenation(concat) => {
                        let mut ids = concat.references().children().map(&mut lower_port_ref);
                        let first = ids.next()?;
                        let last = ids.last().unwrap_or(first);
                        Some(IdxRange::new_inclusive(first..=last))
                    }
                    PortReference(port_ref) => {
                        let id = lower_port_ref(port_ref);
                        Some(IdxRange::new_inclusive(id..=id))
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

                    let (label, src_name) = sub_refs
                        .as_ref()
                        .map(|sub_refs| {
                            let port_ref_id = sub_refs.start();
                            let label = refs.get(port_ref_id).ident.clone();
                            let src_name = ref_srcs
                                .iter()
                                .find_map(|(id, src)| (id == port_ref_id).then_some(src.name))
                                .flatten();
                            (label, src_name)
                        })
                        .unwrap_or((None, None));
                    (NonAnsiPort { label, refs: sub_refs }, src_name)
                }
                EmptyNonAnsiPort(_) => (NonAnsiPort { label: None, refs: None }, None),
            };

            self.region_tree.handle_node(port.syntax());
            let source = SourceAst::new(self.file_id, port).map(NonAnsiPortSrc::from_source_ast);
            let port_id =
                alloc_with_optional_source_entry(&mut ports, &mut port_srcs, hir_port, source);

            // Implicit ports are named by their inner PortReference. Keep the
            // natural AST key for source-to-HIR lookup, then add the named key
            // as the preferred HIR-to-source entry used by navigation.
            if let (Some(source), Some(name)) = (source, src_name) {
                let named_source = NonAnsiPortSrc::new(source.file_id, source.node, Some(name));
                if named_source != source {
                    port_srcs.insert(named_source, port_id);
                }
            }
        }

        self.region_tree.stage(port_list.close_paren(), port_list.syntax());

        self.store.data.ports = Ports::NonAnsi { ports, refs, decls: Arena::default() };
        self.store.sources.port_srcs = PortSrcs::NonAnsi {
            ports: port_srcs,
            refs: ref_srcs,
            decls: SourceMap::default(),
            port_list_src: Some(PortListSrc::NonAnsiPortList(AstId::from_ast(
                self.file_id,
                port_list,
            ))),
        };
    }

    pub(crate) fn lower_port_decl(&mut self, decl: ast::PortDeclaration) -> PortDeclId {
        let header = self.lower_port_header(decl.header(), None);

        let file_id = self.file_id;
        let parent = match (&mut self.store.data.ports, &mut self.store.sources.port_srcs) {
            (Ports::NonAnsi { decls: port_decls, .. }, PortSrcs::NonAnsi { decls: srcs, .. })
            | (Ports::Ansi(port_decls), PortSrcs::Ansi { decls: srcs, .. }) => alloc_with_source(
                file_id,
                port_decls,
                srcs,
                PortDecl { header, decls: empty_decls_range(), name: None },
                decl,
            ),
            _ => unreachable!("port data and source stores use different variants"),
        };

        let decls = self.lower_declarators(decl.declarators(), parent.into());
        match &mut self.store.data.ports {
            Ports::NonAnsi { decls: port_decls, .. } | Ports::Ansi(port_decls) => {
                port_decls[parent].decls = decls;
            }
        }
        parent
    }

    // Port header may inherit properties from the previous port header, so we
    // need to keep track of the previous port header.
    fn lower_port_header(
        &mut self,
        header: ast::PortHeader,
        prev_header: Option<PortHeader>,
    ) -> PortHeader {
        let default_data_ty = DataTy::Builtin(self.db.intern_ty(BuiltinDataTy::default()));
        let default_net_kind = self.default_net_type;
        let prev_header = prev_header.unwrap_or_else(|| self.default_port_header());

        use ast::PortHeader::*;
        let (ast_dir, port_kind, ast_ty) = match header {
            VariablePortHeader(header) => {
                let var_kw = header.var_keyword().map(|_| Either::Left(()));
                (header.direction(), var_kw, header.data_type())
            }
            NetPortHeader(header) => (
                header.direction(),
                lower_net_kind(header.net_type()).map(Either::Right),
                header.data_type(),
            ),
            InterfacePortHeader(_header) => return prev_header,
        };

        let ty_omitted = DataTy::is_ast_missing(ast_ty);
        let all_omitted = ast_dir.is_none() && port_kind.is_none() && ty_omitted;
        let dir = Self::lower_dir(ast_dir).unwrap_or_else(|| prev_header.dir());

        let ty = if !ty_omitted {
            self.lower_data_ty(ast_ty)
        } else if all_omitted {
            prev_header.ty()
        } else {
            default_data_ty
        };

        match port_kind {
            Some(Either::Left(())) => PortHeader::Var { dir, var_kw: true, ty },
            Some(Either::Right(kind)) => PortHeader::Net { dir, net_ty: NetType { kind, ty } },
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
        let default_net_kind = self.default_net_type;
        PortHeader::Net {
            dir: PortDirection::default(),
            net_ty: NetType { kind: default_net_kind, ty: default_data_ty },
        }
    }

    fn lower_dir(tok: Option<SyntaxToken>) -> Option<PortDirection> {
        tok.and_then(|tok| match tok.kind() {
            TokenKind::INPUT_KEYWORD => Some(PortDirection::Input),
            TokenKind::OUTPUT_KEYWORD => Some(PortDirection::Output),
            TokenKind::IN_OUT_KEYWORD => Some(PortDirection::Inout),
            TokenKind::REF_KEYWORD => Some(PortDirection::Ref),
            _ => None,
        })
    }
}
