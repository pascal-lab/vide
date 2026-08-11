use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind,
    ast::{self, AstNode},
};
use triomphe::Arc;

use crate::{
    Ident, alloc_with_source,
    body::{Body, BodySourceMap},
    db::HirDefDb,
    expr::{ExprId, InsideRange, Selector},
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_ident_opt, lower_named_label_opt,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CovergroupDef {
    pub name: Option<Ident>,
    pub options: SmallVec<[ExprId; 4]>,
    pub coverpoints: SmallVec<[CoverpointId; 4]>,
    pub crosses: SmallVec<[CrossId; 2]>,
}

pub type CovergroupId = Idx<CovergroupDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CoverpointDef {
    pub name: Option<Ident>,
    pub expr: ExprId,
    pub iff: Option<ExprId>,
    pub options: SmallVec<[ExprId; 2]>,
    pub bins: SmallVec<[CoverageBin; 4]>,
}

pub type CoverpointId = Idx<CoverpointDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CrossDef {
    pub name: Option<Ident>,
    pub items: SmallVec<[ExprId; 4]>,
    pub iff: Option<ExprId>,
    pub options: SmallVec<[ExprId; 2]>,
    pub bins: SmallVec<[CrossBin; 2]>,
}

pub type CrossId = Idx<CrossDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CoverageBin {
    pub name: Option<Ident>,
    pub wildcard: bool,
    pub size: Option<ExprId>,
    pub initializer: CoverageBinInitializer,
    pub iff: Option<ExprId>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CoverageBinInitializer {
    Default,
    Expr(ExprId),
    Ranges { ranges: Box<[InsideRange]>, with: Option<ExprId> },
    IdWithExpr { with: ExprId },
    Transitions(Box<[TransitionSet]>),
    Unsupported(SyntaxKind),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TransitionSet {
    pub ranges: Box<[TransitionRange]>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TransitionRange {
    pub items: Box<[ExprId]>,
    pub repeat: Option<Selector>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CrossBin {
    pub name: Option<Ident>,
    pub selector: SyntaxKind,
}

pub fn lower_covergroup_decl(covergroup: ast::CovergroupDeclaration<'_>) -> CovergroupDef {
    CovergroupDef {
        name: lower_ident_opt(covergroup.name()),
        options: SmallVec::new(),
        coverpoints: SmallVec::new(),
        crosses: SmallVec::new(),
    }
}

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_covergroup_decl(
        &mut self,
        covergroup_decl: ast::CovergroupDeclaration<'_>,
    ) -> CovergroupId {
        let mut covergroup = lower_covergroup_decl(covergroup_decl);
        for member in covergroup_decl.members().children() {
            match member {
                ast::Member::CoverageOption(option) => {
                    covergroup.options.push(self.lower_expr(option.expr()));
                }
                ast::Member::Coverpoint(coverpoint_ast) => {
                    let coverpoint = self.lower_coverpoint(coverpoint_ast);
                    let coverpoint = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.coverpoints,
                        &mut self.store.sources.coverpoint_srcs,
                        coverpoint,
                        coverpoint_ast,
                    );
                    covergroup.coverpoints.push(coverpoint);
                }
                ast::Member::CoverCross(cross_ast) => {
                    let cross = self.lower_cross(cross_ast);
                    let cross = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.crosses,
                        &mut self.store.sources.cross_srcs,
                        cross,
                        cross_ast,
                    );
                    covergroup.crosses.push(cross);
                }
                ast::Member::EmptyMember(_) => {}
                unsupported => {
                    self.report_unsupported(unsupported.syntax(), "unsupported covergroup member")
                }
            }
        }
        alloc_with_source(
            &self.ast_ids,
            &self.tree,
            &mut self.store.data.covergroups,
            &mut self.store.sources.covergroup_srcs,
            covergroup,
            covergroup_decl,
        )
    }

    fn lower_coverpoint(&mut self, coverpoint: ast::Coverpoint<'_>) -> CoverpointDef {
        let mut result = CoverpointDef {
            name: lower_named_label_opt(coverpoint.label()),
            expr: self.lower_expr(coverpoint.expr()),
            iff: coverpoint.iff().map(|iff| self.lower_expr(iff.expr())),
            options: SmallVec::new(),
            bins: SmallVec::new(),
        };
        for member in coverpoint.members().children() {
            match member {
                ast::Member::CoverageOption(option) => {
                    result.options.push(self.lower_expr(option.expr()))
                }
                ast::Member::CoverageBins(bins) => result.bins.push(self.lower_coverage_bin(bins)),
                ast::Member::EmptyMember(_) => {}
                unsupported => {
                    self.report_unsupported(unsupported.syntax(), "unsupported coverpoint member")
                }
            }
        }
        result
    }

    fn lower_cross(&mut self, cross: ast::CoverCross<'_>) -> CrossDef {
        let mut result = CrossDef {
            name: lower_named_label_opt(cross.label()),
            items: cross
                .items()
                .children()
                .map(|name| self.lower_expr(ast::Expression::Name(name)))
                .collect(),
            iff: cross.iff().map(|iff| self.lower_expr(iff.expr())),
            options: SmallVec::new(),
            bins: SmallVec::new(),
        };
        for member in cross.members().children() {
            match member {
                ast::Member::CoverageOption(option) => {
                    result.options.push(self.lower_expr(option.expr()))
                }
                ast::Member::CoverageBins(bins) => result.bins.push(CrossBin {
                    name: lower_ident_opt(bins.name()),
                    selector: bins.syntax().kind(),
                }),
                ast::Member::EmptyMember(_) => {}
                unsupported => {
                    self.report_unsupported(unsupported.syntax(), "unsupported cross member")
                }
            }
        }
        result
    }

    fn lower_coverage_bin(&mut self, bins: ast::CoverageBins<'_>) -> CoverageBin {
        CoverageBin {
            name: lower_ident_opt(bins.name()),
            wildcard: bins.wildcard().is_some(),
            size: bins.size().and_then(|size| size.expr()).map(|expr| self.lower_expr(expr)),
            initializer: self.lower_coverage_initializer(bins.initializer()),
            iff: bins.iff().map(|iff| self.lower_expr(iff.expr())),
        }
    }

    fn lower_coverage_initializer(
        &mut self,
        initializer: ast::CoverageBinInitializer<'_>,
    ) -> CoverageBinInitializer {
        match initializer {
            ast::CoverageBinInitializer::DefaultCoverageBinInitializer(_) => {
                CoverageBinInitializer::Default
            }
            ast::CoverageBinInitializer::ExpressionCoverageBinInitializer(initializer) => {
                CoverageBinInitializer::Expr(self.lower_expr(initializer.expr()))
            }
            ast::CoverageBinInitializer::RangeCoverageBinInitializer(initializer) => {
                CoverageBinInitializer::Ranges {
                    ranges: initializer
                        .ranges()
                        .value_ranges()
                        .children()
                        .map(|range| self.lower_coverage_range(range))
                        .collect(),
                    with: initializer.with_clause().map(|clause| self.lower_expr(clause.expr())),
                }
            }
            ast::CoverageBinInitializer::IdWithExprCoverageBinInitializer(initializer) => {
                CoverageBinInitializer::IdWithExpr {
                    with: self.lower_expr(initializer.with_clause().expr()),
                }
            }
            ast::CoverageBinInitializer::TransListCoverageBinInitializer(initializer) => {
                CoverageBinInitializer::Transitions(
                    initializer
                        .sets()
                        .children()
                        .map(|set| TransitionSet {
                            ranges: set
                                .ranges()
                                .children()
                                .map(|range| TransitionRange {
                                    items: range
                                        .items()
                                        .children()
                                        .map(|expr| self.lower_expr(expr))
                                        .collect(),
                                    repeat: range.repeat().and_then(|repeat| {
                                        repeat
                                            .selector()
                                            .map(|selector| self.lower_selector(selector))
                                    }),
                                })
                                .collect(),
                        })
                        .collect(),
                )
            }
        }
    }

    fn lower_coverage_range(&mut self, range: ast::Expression) -> InsideRange {
        if let Some(range) = ast::ValueRangeExpression::cast(range.syntax()) {
            return InsideRange::Range {
                left: self.lower_expr(range.left()),
                right: self.lower_expr(range.right()),
            };
        }
        InsideRange::Expr(self.lower_expr(range))
    }
}

pub(crate) fn lower_covergroup_owner(
    db: &dyn HirDefDb,
    owner: OwnerId,
    syntax: &LoweringSyntax,
) -> Arc<Lowered<Body>> {
    debug_assert_eq!(owner.kind(db), OwnerKind::Covergroup);
    let file_id = syntax.file_id;
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let Some(covergroup) = syntax
        .ast_ids
        .node(owner.ast_id(db), &syntax.tree)
        .and_then(ast::CovergroupDeclaration::cast)
    else {
        return Arc::new(Lowered::new(file_id, body, source_map));
    };

    let mut ctx = LoweringCtx::new_with_syntax(
        db,
        owner,
        syntax,
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    ctx.lower_covergroup_decl(covergroup);
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}
