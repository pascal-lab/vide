use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{ast, ast::AstNode, ptr::SyntaxNodePtr};
use utils::define_enum_deriving_from;

use super::LowerModuleCtx;
use crate::{
    hir_def::{
        Ident, alloc_idx_and_src,
        declaration::{DeclarationId, LowerDeclaration},
        expr::{ExprId, LowerExpr},
        lower_ident_opt,
    },
    source_map::{
        AstId, AstKind, FromSourceAst, IsNamedSrc, IsSrc, SourceAst, ToAstNode,
        exact_ast_node_from_ptr,
    },
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SpecifyBlock {
    pub items: SmallVec<[SpecifyBlockItem; 4]>,
}

pub type SpecifyBlockId = Idx<SpecifyBlock>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SpecifyBlockAst;

impl AstKind for SpecifyBlockAst {
    type Node<'a> = ast::SpecifyBlock<'a>;
}

pub type SpecifyBlockSrc = AstId<SpecifyBlockAst>;

impl From<ast::SpecifyBlock<'_>> for SpecifyBlockSrc {
    fn from(block: ast::SpecifyBlock<'_>) -> Self {
        Self::from_ast(block)
    }
}

impl IsNamedSrc for SpecifyBlockSrc {
    fn name_kind(&self) -> Option<syntax::TokenKind> {
        None
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        None
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum SpecifyBlockItem {
        DeclarationId(DeclarationId),
        SpecifyItemId(SpecifyItemId),
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SpecifyItem {
    Path(SpecifyPath),
    ConditionalPath { predicate: ExprId, path: SpecifyPath },
    IfNonePath(SpecifyPath),
    PulseStyle { controls: SmallVec<[ExprId; 2]> },
    TimingCheck { name: Option<Ident>, args: SmallVec<[TimingCheckArg; 6]> },
}

pub type SpecifyItemId = Idx<SpecifyItem>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PathDeclarationAst;

impl AstKind for PathDeclarationAst {
    type Node<'a> = ast::PathDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ConditionalPathDeclarationAst;

impl AstKind for ConditionalPathDeclarationAst {
    type Node<'a> = ast::ConditionalPathDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct IfNonePathDeclarationAst;

impl AstKind for IfNonePathDeclarationAst {
    type Node<'a> = ast::IfNonePathDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PulseStyleDeclarationAst;

impl AstKind for PulseStyleDeclarationAst {
    type Node<'a> = ast::PulseStyleDeclaration<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SystemTimingCheckAst;

impl AstKind for SystemTimingCheckAst {
    type Node<'a> = ast::SystemTimingCheck<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum SpecifyItemSrc {
    PathDeclaration(AstId<PathDeclarationAst>),
    ConditionalPathDeclaration(AstId<ConditionalPathDeclarationAst>),
    IfNonePathDeclaration(AstId<IfNonePathDeclarationAst>),
    PulseStyleDeclaration(AstId<PulseStyleDeclarationAst>),
    SystemTimingCheck(AstId<SystemTimingCheckAst>),
}

impl IsSrc for SpecifyItemSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        SyntaxNodePtr::from(*self).kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        SyntaxNodePtr::from(*self).range()
    }
}

impl<'a> ToAstNode<'a, ast::PathDeclaration<'a>> for SpecifyItemSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PathDeclaration<'a>> {
        let SpecifyItemSrc::PathDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::ConditionalPathDeclaration<'a>> for SpecifyItemSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ConditionalPathDeclaration<'a>> {
        let SpecifyItemSrc::ConditionalPathDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::IfNonePathDeclaration<'a>> for SpecifyItemSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::IfNonePathDeclaration<'a>> {
        let SpecifyItemSrc::IfNonePathDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::PulseStyleDeclaration<'a>> for SpecifyItemSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PulseStyleDeclaration<'a>> {
        let SpecifyItemSrc::PulseStyleDeclaration(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::SystemTimingCheck<'a>> for SpecifyItemSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::SystemTimingCheck<'a>> {
        let SpecifyItemSrc::SystemTimingCheck(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl From<ast::PathDeclaration<'_>> for SpecifyItemSrc {
    fn from(path: ast::PathDeclaration<'_>) -> Self {
        Self::PathDeclaration(AstId::from_ast(path))
    }
}

impl<'a> FromSourceAst<'a, ast::PathDeclaration<'a>> for SpecifyItemSrc {
    fn from_source_ast(path: SourceAst<ast::PathDeclaration<'a>>) -> Self {
        Self::PathDeclaration(AstId::from_source_ast(path))
    }
}

impl From<ast::ConditionalPathDeclaration<'_>> for SpecifyItemSrc {
    fn from(path: ast::ConditionalPathDeclaration<'_>) -> Self {
        Self::ConditionalPathDeclaration(AstId::from_ast(path))
    }
}

impl<'a> FromSourceAst<'a, ast::ConditionalPathDeclaration<'a>> for SpecifyItemSrc {
    fn from_source_ast(path: SourceAst<ast::ConditionalPathDeclaration<'a>>) -> Self {
        Self::ConditionalPathDeclaration(AstId::from_source_ast(path))
    }
}

impl From<ast::IfNonePathDeclaration<'_>> for SpecifyItemSrc {
    fn from(path: ast::IfNonePathDeclaration<'_>) -> Self {
        Self::IfNonePathDeclaration(AstId::from_ast(path))
    }
}

impl<'a> FromSourceAst<'a, ast::IfNonePathDeclaration<'a>> for SpecifyItemSrc {
    fn from_source_ast(path: SourceAst<ast::IfNonePathDeclaration<'a>>) -> Self {
        Self::IfNonePathDeclaration(AstId::from_source_ast(path))
    }
}

impl From<ast::PulseStyleDeclaration<'_>> for SpecifyItemSrc {
    fn from(pulse: ast::PulseStyleDeclaration<'_>) -> Self {
        Self::PulseStyleDeclaration(AstId::from_ast(pulse))
    }
}

impl<'a> FromSourceAst<'a, ast::PulseStyleDeclaration<'a>> for SpecifyItemSrc {
    fn from_source_ast(pulse: SourceAst<ast::PulseStyleDeclaration<'a>>) -> Self {
        Self::PulseStyleDeclaration(AstId::from_source_ast(pulse))
    }
}

impl From<ast::SystemTimingCheck<'_>> for SpecifyItemSrc {
    fn from(timing: ast::SystemTimingCheck<'_>) -> Self {
        Self::SystemTimingCheck(AstId::from_ast(timing))
    }
}

impl<'a> FromSourceAst<'a, ast::SystemTimingCheck<'a>> for SpecifyItemSrc {
    fn from_source_ast(timing: SourceAst<ast::SystemTimingCheck<'a>>) -> Self {
        Self::SystemTimingCheck(AstId::from_source_ast(timing))
    }
}

impl From<SpecifyItemSrc> for SyntaxNodePtr {
    fn from(src: SpecifyItemSrc) -> Self {
        match src {
            SpecifyItemSrc::PathDeclaration(src) => src.ptr(),
            SpecifyItemSrc::ConditionalPathDeclaration(src) => src.ptr(),
            SpecifyItemSrc::IfNonePathDeclaration(src) => src.ptr(),
            SpecifyItemSrc::PulseStyleDeclaration(src) => src.ptr(),
            SpecifyItemSrc::SystemTimingCheck(src) => src.ptr(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SpecifyPath {
    pub inputs: SmallVec<[ExprId; 2]>,
    pub outputs: SmallVec<[ExprId; 2]>,
    pub edge_expr: Option<ExprId>,
    pub delays: SmallVec<[ExprId; 3]>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TimingCheckArg {
    Empty,
    Event { terminal: ExprId, condition: Option<ExprId> },
    Expr(ExprId),
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_specify_block(&mut self, block: ast::SpecifyBlock) -> SpecifyBlockId {
        let items = block
            .items()
            .children()
            .filter_map(|item| {
                use ast::Member::*;
                match item {
                    EmptyMember(_) => None,
                    SpecparamDeclaration(specparam_decl) => {
                        Some(self.declaration_ctx().lower_specparam_decl(specparam_decl).into())
                    }
                    PathDeclaration(path) => Some(self.lower_specify_path_item(path).into()),
                    ConditionalPathDeclaration(path) => {
                        Some(self.lower_conditional_specify_path_item(path).into())
                    }
                    IfNonePathDeclaration(path) => {
                        Some(self.lower_ifnone_specify_path_item(path).into())
                    }
                    PulseStyleDeclaration(pulse) => Some(self.lower_pulse_style_item(pulse).into()),
                    SystemTimingCheck(timing) => {
                        Some(self.lower_system_timing_check_item(timing).into())
                    }
                    _ => None,
                }
            })
            .collect();

        alloc_idx_and_src! {
            SpecifyBlock { items } => self.module.specify_blocks,
            block => self.module_source_map.specify_block_srcs,
        }
    }

    pub(crate) fn lower_specify_path_item(&mut self, path: ast::PathDeclaration) -> SpecifyItemId {
        let item = SpecifyItem::Path(self.lower_specify_path(path));
        alloc_idx_and_src! {
            item => self.module.specify_items,
            path => self.module_source_map.specify_item_srcs,
        }
    }

    pub(crate) fn lower_conditional_specify_path_item(
        &mut self,
        path: ast::ConditionalPathDeclaration,
    ) -> SpecifyItemId {
        let predicate = self.expr_ctx().lower_expr(path.predicate());
        let path_data = self.lower_specify_path(path.path());
        let item = SpecifyItem::ConditionalPath { predicate, path: path_data };

        alloc_idx_and_src! {
            item => self.module.specify_items,
            path => self.module_source_map.specify_item_srcs,
        }
    }

    pub(crate) fn lower_ifnone_specify_path_item(
        &mut self,
        path: ast::IfNonePathDeclaration,
    ) -> SpecifyItemId {
        let item = SpecifyItem::IfNonePath(self.lower_specify_path(path.path()));

        alloc_idx_and_src! {
            item => self.module.specify_items,
            path => self.module_source_map.specify_item_srcs,
        }
    }

    pub(crate) fn lower_pulse_style_item(
        &mut self,
        pulse: ast::PulseStyleDeclaration,
    ) -> SpecifyItemId {
        let controls = pulse.inputs().children().map(|name| self.lower_name_expr(name)).collect();
        let item = SpecifyItem::PulseStyle { controls };

        alloc_idx_and_src! {
            item => self.module.specify_items,
            pulse => self.module_source_map.specify_item_srcs,
        }
    }

    pub(crate) fn lower_system_timing_check_item(
        &mut self,
        timing: ast::SystemTimingCheck,
    ) -> SpecifyItemId {
        let name = lower_ident_opt(timing.name());
        let args = timing.args().children().map(|arg| self.lower_timing_check_arg(arg)).collect();
        let item = SpecifyItem::TimingCheck { name, args };

        alloc_idx_and_src! {
            item => self.module.specify_items,
            timing => self.module_source_map.specify_item_srcs,
        }
    }

    fn lower_specify_path(&mut self, path: ast::PathDeclaration) -> SpecifyPath {
        let desc = path.desc();
        let inputs = desc.inputs().children().map(|name| self.lower_name_expr(name)).collect();
        let (outputs, edge_expr) = match desc.suffix() {
            ast::PathSuffix::SimplePathSuffix(suffix) => {
                let outputs =
                    suffix.outputs().children().map(|name| self.lower_name_expr(name)).collect();
                (outputs, None)
            }
            ast::PathSuffix::EdgeSensitivePathSuffix(suffix) => {
                let outputs =
                    suffix.outputs().children().map(|name| self.lower_name_expr(name)).collect();
                (outputs, Some(self.expr_ctx().lower_expr(suffix.expr())))
            }
        };
        let delays =
            path.delays().children().map(|expr| self.expr_ctx().lower_expr(expr)).collect();

        SpecifyPath { inputs, outputs, edge_expr, delays }
    }

    fn lower_timing_check_arg(&mut self, arg: ast::TimingCheckArg) -> TimingCheckArg {
        use ast::TimingCheckArg::*;
        match arg {
            EmptyTimingCheckArg(_) => TimingCheckArg::Empty,
            TimingCheckEventArg(arg) => {
                let terminal = self.expr_ctx().lower_expr(arg.terminal());
                let condition = arg.condition().map(|cond| self.expr_ctx().lower_expr(cond.expr()));
                TimingCheckArg::Event { terminal, condition }
            }
            ExpressionTimingCheckArg(arg) => {
                TimingCheckArg::Expr(self.expr_ctx().lower_expr(arg.expr()))
            }
        }
    }

    fn lower_name_expr(&mut self, name: ast::Name) -> ExprId {
        ast::Expression::cast(name.syntax())
            .map(|expr| self.expr_ctx().lower_expr(expr))
            .unwrap_or_else(|| self.expr_ctx().lower_expr_opt(None))
    }
}
