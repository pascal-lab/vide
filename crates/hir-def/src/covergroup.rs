use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};
use triomphe::Arc;

use crate::{
    Ident, alloc_with_source,
    body::{Body, BodySourceMap},
    db::HirDefDb,
    lower::{BodyStore, LoweringCtx, LoweringSyntax},
    lower_ident_opt, lower_named_label_opt,
    owner::{OwnerId, OwnerKind},
    source_map::Lowered,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CovergroupDef {
    pub name: Option<Ident>,
    pub coverpoints: SmallVec<[CoverpointId; 4]>,
    pub crosses: SmallVec<[CrossId; 2]>,
}

pub type CovergroupId = Idx<CovergroupDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CoverpointDef {
    pub name: Option<Ident>,
}

pub type CoverpointId = Idx<CoverpointDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CrossDef {
    pub name: Option<Ident>,
}

pub type CrossId = Idx<CrossDef>;

pub fn lower_covergroup_decl(covergroup: ast::CovergroupDeclaration<'_>) -> CovergroupDef {
    CovergroupDef {
        name: lower_ident_opt(covergroup.name()),
        coverpoints: SmallVec::new(),
        crosses: SmallVec::new(),
    }
}

pub fn lower_coverpoint(coverpoint: ast::Coverpoint<'_>) -> CoverpointDef {
    CoverpointDef { name: lower_named_label_opt(coverpoint.label()) }
}

pub fn lower_cross(cross: ast::CoverCross<'_>) -> CrossDef {
    CrossDef { name: lower_named_label_opt(cross.label()) }
}

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_covergroup_decl(
        &mut self,
        covergroup_decl: ast::CovergroupDeclaration<'_>,
    ) -> CovergroupId {
        let mut covergroup = lower_covergroup_decl(covergroup_decl);
        for member in covergroup_decl.members().children() {
            match member {
                ast::Member::Coverpoint(coverpoint_ast) => {
                    let coverpoint = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.coverpoints,
                        &mut self.store.sources.coverpoint_srcs,
                        lower_coverpoint(coverpoint_ast),
                        coverpoint_ast,
                    );
                    covergroup.coverpoints.push(coverpoint);
                }
                ast::Member::CoverCross(cross_ast) => {
                    let cross = alloc_with_source(
                        &self.ast_ids,
                        &self.tree,
                        &mut self.store.data.crosses,
                        &mut self.store.sources.cross_srcs,
                        lower_cross(cross_ast),
                        cross_ast,
                    );
                    covergroup.crosses.push(cross);
                }
                _ => {}
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

    let mut ctx = LoweringCtx::new_with_syntax(db, 
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
