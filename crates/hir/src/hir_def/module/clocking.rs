use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use super::{LowerModuleCtx, port::PortDirection};
use crate::{
    hir_def::{Ident, alloc_with_source, expr::timing_control::EventExprId, lower_ident_opt},
    source_map::{FromSourceAst, IsNamedSrc, IsSrc, SourceAst, root_token_in},
};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ClockingBlockKind {
    #[default]
    Regular,
    Default,
    Global,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ClockingBlockDef {
    pub name: Option<Ident>,
    pub kind: ClockingBlockKind,
    pub event: EventExprId,
    pub signals: SmallVec<[ClockingSignal; 4]>,
}

pub type ClockingBlockId = Idx<ClockingBlockDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ClockingSignal {
    pub name: Ident,
    pub dir: PortDirection,
    pub name_range: Option<TextRange>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct ClockingSignalId(pub u32);
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DefaultClockingRef {
    pub name: Option<Ident>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DefaultClockingRefSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for DefaultClockingRefSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for DefaultClockingRefSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> FromSourceAst<'a, ast::DefaultClockingReference<'a>> for DefaultClockingRefSrc {
    fn from_source_ast(reference: SourceAst<ast::DefaultClockingReference<'a>>) -> Self {
        let reference = reference.into_inner();
        let syntax = reference.syntax();
        let name = reference
            .name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&reference), name }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ClockingBlockSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for ClockingBlockSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for ClockingBlockSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> FromSourceAst<'a, ast::ClockingDeclaration<'a>> for ClockingBlockSrc {
    fn from_source_ast(clocking: SourceAst<ast::ClockingDeclaration<'a>>) -> Self {
        let clocking = clocking.into_inner();
        let syntax = clocking.syntax();
        let name = clocking
            .block_name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&clocking), name }
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_clocking_declaration(
        &mut self,
        clocking: ast::ClockingDeclaration<'_>,
    ) -> ClockingBlockId {
        let name = lower_ident_opt(clocking.block_name());
        let kind = match clocking.global_or_default().map(|token| token.kind()) {
            Some(TokenKind::DEFAULT_KEYWORD) => ClockingBlockKind::Default,
            Some(TokenKind::GLOBAL_KEYWORD) => ClockingBlockKind::Global,
            _ => ClockingBlockKind::Regular,
        };
        let event = self.lower_event_expr(clocking.event());
        let signals = lower_clocking_signals(clocking);
        let file_id = self.file_id;
        let (clocking_blocks, sources) =
            (&mut self.store.data.clocking_blocks, &mut self.store.sources.clocking_block_srcs);
        alloc_with_source(
            file_id,
            clocking_blocks,
            sources,
            ClockingBlockDef { name, kind, event, signals },
            clocking,
        )
    }

    pub(crate) fn lower_default_clocking_reference(
        &mut self,
        reference: ast::DefaultClockingReference<'_>,
    ) {
        let source =
            SourceAst::new(self.file_id, reference).map(DefaultClockingRefSrc::from_source_ast);
        self.store.data.default_clocking =
            Some(DefaultClockingRef { name: lower_ident_opt(reference.name()) });
        self.store.sources.default_clocking_src = source;
    }
}

fn lower_clocking_signals(clocking: ast::ClockingDeclaration<'_>) -> SmallVec<[ClockingSignal; 4]> {
    let mut signals = SmallVec::new();
    let syntax = clocking.syntax();
    for item in clocking.items().children() {
        let ast::Member::ClockingItem(item) = item else {
            continue;
        };
        let dir = lower_clocking_direction(item.direction());
        for decl in item.decls().children() {
            let name_range = decl.name().and_then(|name| root_token_in(syntax, name)?.text_range());
            let Some(name) = lower_ident_opt(decl.name()) else {
                continue;
            };
            signals.push(ClockingSignal { name, dir, name_range });
        }
    }
    signals
}

fn lower_clocking_direction(direction: ast::ClockingDirection<'_>) -> PortDirection {
    if direction.output().is_some() { PortDirection::Output } else { PortDirection::Input }
}
