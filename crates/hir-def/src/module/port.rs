use itertools::Either;
use la_arena::{Arena, Idx, IdxRange};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    SyntaxToken, TokenKind,
    ast::{self, AstNode, PortExpression},
    has_text_range::HasTextRange,
};
use utils::{
    get::{Get, GetRef},
    text_edit::TextSize,
};

use crate::{
    Ident, alloc_with_source, alloc_with_source_entry,
    ast_id_map::SourceAstId,
    declaration::Declaration,
    expr::{
        Selector,
        data_ty::{BuiltinDataTy, BuiltinDataTyId, DataTy},
        declarator::{DeclId, DeclsRange, empty_decls_range},
    },
    lower_ident_opt,
    module::LowerModuleCtx,
    source_map::SourceMap,
    ty::{NetKind, NetType, lower_net_kind},
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

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum PortDirection {
    Input,
    Output,
    Ref,
    #[default]
    Inout,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortHeader {
    Var { dir: PortDirection, var_kw: bool, ty: DataTy },
    Net { dir: PortDirection, net_ty: NetType },
    /// A generic interface port (`interface.port_name`). The port direction is
    /// not part of the header syntax and is only inherited from a preceding
    /// interface header.
    Interface { dir: PortDirection },
}

impl PortHeader {
    pub fn dir(&self) -> PortDirection {
        match self {
            PortHeader::Var { dir, .. }
            | PortHeader::Net { dir, .. }
            | PortHeader::Interface { dir } => *dir,
        }
    }

    pub fn ty(&self) -> DataTy {
        match self {
            PortHeader::Var { ty, .. } => ty.clone(),
            PortHeader::Net { net_ty: NetType { ty, .. }, .. } => ty.clone(),
            // Interface ports carry no data type; report the default rather
            // than the unrelated previous port header.
            PortHeader::Interface { .. } => {
                DataTy::Builtin(BuiltinDataTyId::new(BuiltinDataTy::default()))
            }
        }
    }
}
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct NonAnsiPortBindings {
    pub decl_to_port: FxHashMap<DeclId, NonAnsiPortId>,
    pub origins_by_port: FxHashMap<NonAnsiPortId, SmallVec<[DeclId; 2]>>,
}
fn non_ansi_binding_role(body: &crate::body::Body, decl_id: DeclId) -> Option<u8> {
    match body.decls[decl_id].parent {
        crate::expr::declarator::DeclaratorParent::PortDeclId(_) => Some(0),
        crate::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) => matches!(
            body.declarations[declaration_id],
            Declaration::DataDecl(_) | Declaration::NetDecl(_)
        )
        .then_some(1),
        crate::expr::declarator::DeclaratorParent::StmtId(_) => None,
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Ports {
    NonAnsi {
        ports: Arena<NonAnsiPort>,
        refs: Arena<PortRef>,
        decls: Arena<PortDecl>,
        bindings: NonAnsiPortBindings,
    },
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
            Ports::NonAnsi { ports, refs, decls, bindings } => {
                ports.shrink_to_fit();
                refs.shrink_to_fit();
                decls.shrink_to_fit();
                bindings.decl_to_port.shrink_to_fit();
                bindings.origins_by_port.shrink_to_fit();
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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct PortRef {
    pub ident: Option<Ident>,
    pub select: Option<Selector>,
}

pub type PortRefId = Idx<PortRef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PortSrcs {
    NonAnsi {
        ports: SourceMap<NonAnsiPort>,
        refs: SourceMap<PortRef>,
        decls: SourceMap<PortDecl>,
        port_list_src: Option<SourceAstId>,
    },
    Ansi {
        decls: SourceMap<PortDecl>,
        port_list_src: Option<SourceAstId>,
    },
}

impl PortSrcs {
    pub fn port_list_src(&self) -> Option<SourceAstId> {
        match self {
            PortSrcs::NonAnsi { port_list_src, .. } | PortSrcs::Ansi { port_list_src, .. } => {
                *port_list_src
            }
        }
    }
}

impl Default for PortSrcs {
    fn default() -> Self {
        PortSrcs::Ansi { decls: SourceMap::default(), port_list_src: None }
    }
}

impl Get<NonAnsiPortId> for PortSrcs {
    type Output = Option<SourceAstId>;

    fn get(&self, port_id: NonAnsiPortId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { ports, .. } => ports.get(port_id),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<PortRefId> for PortSrcs {
    type Output = Option<SourceAstId>;

    fn get(&self, port_ref_id: PortRefId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { refs, .. } => refs.get(port_ref_id),
            PortSrcs::Ansi { .. } => None,
        }
    }
}

impl Get<PortDeclId> for PortSrcs {
    type Output = Option<SourceAstId>;

    fn get(&self, port_id: PortDeclId) -> Self::Output {
        match self {
            PortSrcs::NonAnsi { decls, .. } => decls.get(port_id),
            PortSrcs::Ansi { decls, .. } => decls.get(port_id),
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
        let mut decls: SourceMap<PortDecl> = SourceMap::default();

        let mut header = None;
        for port in port_list.ports().children() {
            use ast::Member::*;
            match port {
                ImplicitAnsiPort(port) => {
                    // `header` carries the previous port header (None for the
                    // first port); the lowerer resolves the default itself.
                    let current_header = self.lower_port_header(port.header(), header);
                    header = Some(current_header.clone());
                    let parent = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut ports,
                        &mut decls,
                        PortDecl { header: current_header, decls: empty_decls_range(), name: None },
                        port,
                    );
                    let decl_id = self.lower_declarator(port.declarator(), parent.into());
                    ports[parent].decls = IdxRange::new_inclusive(decl_id..=decl_id);
                }
                ExplicitAnsiPort(port) => {
                    let offset = port
                        .syntax()
                        .text_range()
                        .map(|range| range.start())
                        .unwrap_or_default();
                    let current_header =
                        self.lower_explicit_ansi_header(port.direction(), header, offset);
                    if let Some(expr) = port.expr() {
                        self.lower_expr(expr);
                    }
                    header = Some(current_header.clone());
                    alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
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
            }
        }

        let port_list_src = self.source_id(port_list.syntax());
        self.store.data.ports = Ports::Ansi(ports);
        self.store.sources.port_srcs = PortSrcs::Ansi { decls, port_list_src: Some(port_list_src) };
    }

    pub(crate) fn lower_wildcard_ports(&mut self, port_list: ast::WildcardPortList) {
        let port_list_src = self.source_id(port_list.syntax());
        self.store.data.ports = Ports::Ansi(Arena::default());
        self.store.sources.port_srcs =
            PortSrcs::Ansi { decls: SourceMap::default(), port_list_src: Some(port_list_src) };
    }

    pub(crate) fn lower_nonansi_port(&mut self, port_list: ast::NonAnsiPortList) {
        let mut ports = Arena::default();
        let mut refs = Arena::default();
        let mut port_srcs = SourceMap::default();
        let mut ref_srcs = SourceMap::default();

        for port in port_list.ports().children() {
            use ast::{NonAnsiPort::*, PortExpression::*};

            let mut lower_port_exprs = |exprs: Option<PortExpression>| {
                let mut lower_port_ref = |port_ref: ast::PortReference| {
                    let ident = lower_ident_opt(port_ref.name());
                    let select = port_ref
                        .select()
                        .and_then(|select| select.selector())
                        .map(|select| self.lower_selector(select));
                    alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
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

            let hir_port = match port {
                ExplicitNonAnsiPort(port) => NonAnsiPort {
                    label: lower_ident_opt(port.name()),
                    refs: lower_port_exprs(port.expr()),
                },
                ImplicitNonAnsiPort(port) => {
                    let port_refs = lower_port_exprs(Some(port.expr()));
                    debug_assert!(port_refs.as_ref().is_none_or(|refs| refs.len() == 1));
                    let label = port_refs
                        .as_ref()
                        .map(|range| &refs[range.start()])
                        .and_then(|port_ref| port_ref.ident.clone());
                    NonAnsiPort { label, refs: port_refs }
                }
                EmptyNonAnsiPort(_) => NonAnsiPort { label: None, refs: None },
            };

            let source = self.source_id(port.syntax());
            alloc_with_source_entry(&mut ports, &mut port_srcs, hir_port, source);
        }

        let port_list_src = self.source_id(port_list.syntax());
        self.store.data.ports = Ports::NonAnsi {
            ports,
            refs,
            decls: Arena::default(),
            bindings: NonAnsiPortBindings::default(),
        };
        self.store.sources.port_srcs = PortSrcs::NonAnsi {
            ports: port_srcs,
            refs: ref_srcs,
            decls: SourceMap::default(),
            port_list_src: Some(port_list_src),
        };
    }

    pub(crate) fn lower_port_decl(&mut self, decl: ast::PortDeclaration) -> PortDeclId {
        let header = self.lower_port_header(decl.header(), None);
        let source = self.source_id(decl.syntax());
        let parent = match (&mut self.store.data.ports, &mut self.store.sources.port_srcs) {
            (Ports::NonAnsi { decls: port_decls, .. }, PortSrcs::NonAnsi { decls: srcs, .. })
            | (Ports::Ansi(port_decls), PortSrcs::Ansi { decls: srcs, .. }) => {
                alloc_with_source_entry(
                    port_decls,
                    srcs,
                    PortDecl { header, decls: empty_decls_range(), name: None },
                    source,
                )
            }
            _ => unreachable!("port data and source stores use different variants"),
        };

        let decls = self.lower_declarators(decl.declarators(), parent.into());
        match &mut self.store.data.ports {
            Ports::NonAnsi { decls: port_decls, .. } | Ports::Ansi(port_decls) => {
                port_decls[parent].decls = decls.clone();
            }
        }
        self.bind_nonansi_declarations(decls);
        parent
    }

    pub(crate) fn bind_nonansi_declarations<I>(&mut self, decl_ids: I)
    where
        I: IntoIterator<Item = DeclId>,
    {
        let decl_ids: Vec<_> = decl_ids.into_iter().collect();
        let body = &mut self.store.data;
        for decl_id in decl_ids {
            let Some(role) = non_ansi_binding_role(body, decl_id) else {
                continue;
            };
            let Some(name) = body.decls[decl_id].name.clone() else {
                continue;
            };
            let port_id = {
                let Ports::NonAnsi { ports, .. } = &body.ports else {
                    continue;
                };
                let mut matches = ports.iter().filter_map(|(port_id, port)| {
                    (port.label.as_ref() == Some(&name)).then_some(port_id)
                });
                let Some(port_id) = matches.next() else {
                    continue;
                };
                if matches.next().is_some() {
                    continue;
                }
                port_id
            };
            let conflicting: Vec<_> = {
                let Ports::NonAnsi { bindings, .. } = &body.ports else {
                    continue;
                };
                bindings
                    .origins_by_port
                    .get(&port_id)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|existing| non_ansi_binding_role(body, *existing) == Some(role))
                    .collect()
            };
            let Ports::NonAnsi { bindings, .. } = &mut body.ports else {
                continue;
            };
            if !conflicting.is_empty() {
                if let Some(origins) = bindings.origins_by_port.get_mut(&port_id) {
                    origins.retain(|existing| !conflicting.contains(existing));
                }
                for existing in conflicting {
                    bindings.decl_to_port.remove(&existing);
                }
                continue;
            }
            bindings.decl_to_port.insert(decl_id, port_id);
            bindings.origins_by_port.entry(port_id).or_default().push(decl_id);
        }
    }

    // Port header may inherit properties from the previous port header, so we
    // need to keep track of the previous port header.
    fn lower_port_header(
        &mut self,
        header: ast::PortHeader,
        prev_header: Option<PortHeader>,
    ) -> PortHeader {
        let default_data_ty = DataTy::Builtin(BuiltinDataTyId::new(BuiltinDataTy::default()));
        let header_node = header.syntax();
        let header_offset = header_node.text_range().map(|range| range.start()).unwrap_or_default();
        let prev_header =
            prev_header.unwrap_or_else(|| self.default_port_header(header_offset));

        use ast::PortHeader::*;
        // A generic interface port carries no net/var header and no direction
        // syntax; only its direction inherits from a preceding interface port.
        if let InterfacePortHeader(_) = header {
            let dir = match prev_header {
                PortHeader::Interface { dir } => dir,
                _ => PortDirection::default(),
            };
            return PortHeader::Interface { dir };
        }
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
            InterfacePortHeader(_) => unreachable!("handled above"),
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
                // An input/inout port is an implicit net only when its type is
                // implicit (no type and nothing inherited); an explicit data
                // type (e.g. `input logic a`) makes it a variable port.
                let implicit = ty_omitted && !all_omitted;
                if (implicit && matches!(dir, PortDirection::Input | PortDirection::Inout))
                    || (matches!(dir, PortDirection::Output)
                        && matches!(ast_ty, ast::DataType::ImplicitType(_)))
                {
                    let kind = self.implicit_net_kind(header_node);
                    PortHeader::Net { dir, net_ty: NetType { kind, ty } }
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
        offset: TextSize,
    ) -> PortHeader {
        let dir = Self::lower_dir(direction);
        let prev_header = prev_header.unwrap_or_else(|| self.default_port_header(offset));
        let Some(dir) = dir else {
            return prev_header;
        };

        match prev_header {
            PortHeader::Var { var_kw, ty, .. } => PortHeader::Var { dir, var_kw, ty },
            PortHeader::Net { net_ty, .. } => PortHeader::Net { dir, net_ty },
            PortHeader::Interface { .. } => PortHeader::Interface { dir },
        }
    }

    fn default_port_header(&mut self, offset: TextSize) -> PortHeader {
        let default_data_ty = DataTy::Builtin(BuiltinDataTyId::new(BuiltinDataTy::default()));
        let kind = self.net_kind_at(offset).unwrap_or(NetKind::Wire);
        PortHeader::Net {
            dir: PortDirection::default(),
            net_ty: NetType { kind, ty: default_data_ty },
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
