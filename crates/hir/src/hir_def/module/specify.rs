use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{ast, ast::AstNode, ptr::SyntaxNodePtr};
use utils::define_enum_deriving_from;

use super::LowerModuleCtx;
use crate::{
    define_src,
    hir_def::{
        Ident, alloc_idx_and_src,
        declaration::{DeclarationId, LowerDeclaration},
        expr::{ExprId, LowerExpr},
        lower_ident_opt,
    },
    source_map::IsNamedSrc,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SpecifyBlock {
    pub items: SmallVec<[SpecifyBlockItem; 4]>,
}

pub type SpecifyBlockId = Idx<SpecifyBlock>;
define_src!(SpecifyBlockSrc(ast::SpecifyBlock));

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

define_src!(SpecifyItemSrc(
    ast::PathDeclaration,
    ast::ConditionalPathDeclaration,
    ast::IfNonePathDeclaration,
    ast::PulseStyleDeclaration,
    ast::SystemTimingCheck
));

impl From<SpecifyItemSrc> for SyntaxNodePtr {
    fn from(src: SpecifyItemSrc) -> Self {
        match src {
            SpecifyItemSrc::PathDeclaration(ptr)
            | SpecifyItemSrc::ConditionalPathDeclaration(ptr)
            | SpecifyItemSrc::IfNonePathDeclaration(ptr)
            | SpecifyItemSrc::PulseStyleDeclaration(ptr)
            | SpecifyItemSrc::SystemTimingCheck(ptr) => ptr,
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
