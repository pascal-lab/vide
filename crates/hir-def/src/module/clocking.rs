use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxTokenWithParent, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use triomphe::Arc;
use utils::text_edit::TextRange;

use super::{LowerModuleCtx, port::PortDirection};
use crate::{
    Ident, alloc_with_source,
    body::{Body, BodySourceMap},
    db::HirDefDb,
    expr::timing_control::EventExprId,
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
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
    pub source: crate::ast_id_map::SourceAstId,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct ClockingSignalId(pub u32);
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DefaultClockingRef {
    pub name: Option<Ident>,
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
        let signals = self.lower_clocking_signals(clocking);
        let (clocking_blocks, sources) =
            (&mut self.store.data.clocking_blocks, &mut self.store.sources.clocking_block_srcs);
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
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
        let source = self.source_id(reference.syntax());
        self.store.data.default_clocking =
            Some(DefaultClockingRef { name: lower_ident_opt(reference.name()) });
        self.store.sources.default_clocking_src = Some(source);
    }

    fn lower_clocking_signals(
        &self,
        clocking: ast::ClockingDeclaration<'_>,
    ) -> SmallVec<[ClockingSignal; 4]> {
        let mut signals = SmallVec::new();
        let syntax = clocking.syntax();
        for item in clocking.items().children() {
            let ast::Member::ClockingItem(item) = item else {
                continue;
            };
            let dir = lower_clocking_direction(item.direction());
            for decl in item.decls().children() {
                let name_range = decl.name().and_then(|name| {
                    SyntaxTokenWithParent { parent: syntax, tok: name }.text_range()
                });
                let Some(name) = lower_ident_opt(decl.name()) else {
                    continue;
                };
                signals.push(ClockingSignal {
                    name,
                    dir,
                    name_range,
                    source: self.source_id(decl.syntax()),
                });
            }
        }
        signals
    }
}

fn lower_clocking_direction(direction: ast::ClockingDirection<'_>) -> PortDirection {
    if direction.output().is_some() { PortDirection::Output } else { PortDirection::Input }
}

pub(crate) fn lower_clocking_owner(
    db: &dyn HirDefDb,
    owner: OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::ClockingBlock);
    let file_id = syntax.file_id;
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let Some(clocking) = syntax
        .ast_ids
        .node(owner.ast_id(db), &syntax.tree)
        .and_then(ast::ClockingDeclaration::cast)
    else {
        return Arc::new(Lowered::new(file_id, body, source_map));
    };

    let mut ctx = LoweringCtx::new_with_syntax(
        owner,
        syntax,
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    ctx.lower_clocking_declaration(clocking);
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}
