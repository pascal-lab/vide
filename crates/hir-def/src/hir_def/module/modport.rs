use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, SyntaxToken, TokenKind,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use super::{LowerModuleCtx, port::PortDirection};
use crate::{
    hir_def::{Ident, alloc_with_source, lower_ident_opt},
    source_map::{FromSourceAst, IsNamedSrc, IsSrc, SourceAst, root_token_in},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModportDef {
    pub name: Option<Ident>,
    pub ports: SmallVec<[ModportPort; 4]>,
}

pub type ModportId = Idx<ModportDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ModportPort {
    pub name: Ident,
    pub dir: PortDirection,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ModportSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for ModportSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for ModportSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> FromSourceAst<'a, ast::ModportItem<'a>> for ModportSrc {
    fn from_source_ast(item: SourceAst<ast::ModportItem<'a>>) -> Self {
        let item = item.into_inner();
        let syntax = item.syntax();
        let name = item
            .name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&item), name }
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_modport_declaration(
        &mut self,
        modport: ast::ModportDeclaration<'_>,
    ) -> SmallVec<[ModportId; 1]> {
        let mut lowered = SmallVec::new();
        for item in modport.items().children() {
            let name = lower_ident_opt(item.name());
            let ports = lower_modport_ports(item);
            let file_id = self.file_id;
            let (modports, sources) =
                (&mut self.store.data.modports, &mut self.store.sources.modport_srcs);
            let modport_id =
                alloc_with_source(file_id, modports, sources, ModportDef { name, ports }, item);
            lowered.push(modport_id);
        }
        lowered
    }
}

fn lower_modport_ports(item: ast::ModportItem<'_>) -> SmallVec<[ModportPort; 4]> {
    let mut ports = SmallVec::new();

    // slang models a modport item as named entries whose `ports()` members are
    // simple or subroutine port lists; each list then owns its inner ports.
    for member in item.ports().ports().children() {
        match member {
            ast::Member::ModportSimplePortList(port_list) => {
                let dir = lower_modport_dir(port_list.direction()).unwrap_or_default();
                for port in port_list.ports().children() {
                    if let Some(name) = lower_modport_port_name(port) {
                        ports.push(ModportPort { name, dir });
                    }
                }
            }
            ast::Member::ModportSubroutinePortList(port_list) => {
                let dir = lower_modport_subroutine_dir(port_list.import_export());
                for port in port_list.ports().children() {
                    if let Some(name) = lower_modport_port_name(port) {
                        ports.push(ModportPort { name, dir });
                    }
                }
            }
            _ => {}
        }
    }

    ports
}

fn lower_modport_port_name(port: ast::ModportPort<'_>) -> Option<Ident> {
    match port {
        ast::ModportPort::ModportNamedPort(port) => lower_ident_opt(port.name()),
        ast::ModportPort::ModportExplicitPort(port) => lower_ident_opt(port.name()),
        ast::ModportPort::ModportSubroutinePort(port) => {
            lower_ident_opt(rightmost_name_token(port.prototype().name()))
        }
    }
}

fn rightmost_name_token(name: ast::Name<'_>) -> Option<SyntaxToken<'_>> {
    match name {
        ast::Name::IdentifierName(name) => name.identifier(),
        ast::Name::IdentifierSelectName(name) => name.identifier(),
        ast::Name::ScopedName(name) => rightmost_name_token(name.right()),
        _ => None,
    }
}

fn lower_modport_dir(tok: Option<SyntaxToken<'_>>) -> Option<PortDirection> {
    tok.and_then(|tok| match tok.kind() {
        TokenKind::INPUT_KEYWORD => Some(PortDirection::Input),
        TokenKind::OUTPUT_KEYWORD => Some(PortDirection::Output),
        TokenKind::IN_OUT_KEYWORD => Some(PortDirection::Inout),
        TokenKind::REF_KEYWORD => Some(PortDirection::Ref),
        _ => None,
    })
}

fn lower_modport_subroutine_dir(tok: Option<SyntaxToken<'_>>) -> PortDirection {
    match tok.map(|tok| tok.kind()) {
        Some(TokenKind::EXPORT_KEYWORD) => PortDirection::Output,
        _ => PortDirection::Input,
    }
}
