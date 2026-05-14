use syntax::{
    SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
};
use utils::text_edit::TextRange;

use crate::{
    hir_def::{Ident, lower_ident_opt},
    source_map::{IsNamedSrc, IsSrc},
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum OpaqueKind {
    FileItem,
    ModuleItem,
    BlockItem,
    Generate,
    Udp,
    Specify,
    Config,
    Library,
    DefParam,
    Specparam,
    Genvar,
    Statement,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct OpaqueItem {
    pub name: Option<Ident>,
    pub kind: OpaqueKind,
}

pub type OpaqueItemId = la_arena::Idx<OpaqueItem>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct OpaqueItemSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
    pub kind: OpaqueKind,
}

impl IsSrc for OpaqueItemSrc {
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for OpaqueItemSrc {
    fn name_kind(&self) -> Option<syntax::TokenKind> {
        self.name.map(|name| name.kind())
    }

    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl From<OpaqueItemSrc> for SyntaxNodePtr {
    fn from(src: OpaqueItemSrc) -> Self {
        src.node
    }
}

pub(crate) fn lower_opaque_node(
    node: SyntaxNode<'_>,
    name_token: Option<SyntaxToken<'_>>,
    kind: OpaqueKind,
) -> (OpaqueItem, OpaqueItemSrc) {
    let name = lower_ident_opt(name_token);
    let src = OpaqueItemSrc {
        node: SyntaxNodePtr::from_node(node),
        name: name_token.map(SyntaxTokenPtr::from_token),
        kind,
    };
    (OpaqueItem { name, kind }, src)
}

pub(crate) fn lower_opaque_member(member: ast::Member<'_>) -> (OpaqueItem, OpaqueItemSrc) {
    let kind = opaque_member_kind(member);
    let name = opaque_member_name(member);
    lower_opaque_node(member.syntax(), name, kind)
}

pub(crate) fn opaque_member_kind(member: ast::Member<'_>) -> OpaqueKind {
    use ast::Member::*;
    match member {
        GenerateRegion(_) | GenerateBlock(_) | IfGenerate(_) | CaseGenerate(_)
        | LoopGenerate(_) => OpaqueKind::Generate,
        UdpDeclaration(_) | ExternUdpDecl(_) => OpaqueKind::Udp,
        SpecifyBlock(_)
        | PathDeclaration(_)
        | ConditionalPathDeclaration(_)
        | IfNonePathDeclaration(_)
        | SystemTimingCheck(_)
        | PulseStyleDeclaration(_)
        | DefaultSkewItem(_) => OpaqueKind::Specify,
        ConfigDeclaration(_) => OpaqueKind::Config,
        LibraryDeclaration(_) | LibraryIncludeStatement(_) => OpaqueKind::Library,
        DefParam(_) => OpaqueKind::DefParam,
        SpecparamDeclaration(_) => OpaqueKind::Specparam,
        GenvarDeclaration(_) => OpaqueKind::Genvar,
        _ => OpaqueKind::ModuleItem,
    }
}

pub(crate) fn opaque_member_name(member: ast::Member<'_>) -> Option<SyntaxToken<'_>> {
    use ast::Member::*;
    match member {
        ModuleDeclaration(item) => item.header().name(),
        UdpDeclaration(item) => item.name(),
        ConfigDeclaration(item) => item.name(),
        LibraryDeclaration(item) => item.name(),
        GenerateBlock(item) => item
            .label()
            .and_then(|label| label.name())
            .or_else(|| item.begin_name().and_then(|name| name.name())),
        IfGenerate(item) => opaque_member_name(item.block()),
        LoopGenerate(item) => opaque_member_name(item.block()),
        GenvarDeclaration(item) => {
            item.identifiers().children().next().and_then(|id| id.identifier())
        }
        SpecparamDeclaration(item) => {
            item.declarators().children().next().and_then(|decl| decl.name())
        }
        _ => None,
    }
}
