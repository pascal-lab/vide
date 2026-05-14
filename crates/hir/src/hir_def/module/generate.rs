use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast;

use super::{LowerModuleCtx, ModuleItem};
use crate::{
    define_src,
    hir_def::{alloc_idx_and_src, declaration::LowerDeclaration},
    source_map::IsNamedSrc,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenerateRegion {
    pub items: SmallVec<[ModuleItem; 4]>,
}

pub type GenerateRegionId = Idx<GenerateRegion>;
define_src!(GenerateRegionSrc(ast::GenerateRegion));

impl IsNamedSrc for GenerateRegionSrc {
    fn name_kind(&self) -> Option<syntax::TokenKind> {
        None
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        None
    }
}

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_generate_region(
        &mut self,
        region: ast::GenerateRegion,
    ) -> GenerateRegionId {
        let items = region
            .members()
            .children()
            .filter_map(|item| {
                use ast::Member::*;
                match item {
                    EmptyMember(_) => None,
                    GenvarDeclaration(genvar_decl) => {
                        Some(self.declaration_ctx().lower_genvar_decl(genvar_decl).into())
                    }
                    item => Some(self.lower_opaque_member(item).into()),
                }
            })
            .collect();

        alloc_idx_and_src! {
            GenerateRegion { items } => self.module.generate_regions,
            region => self.module_source_map.generate_region_srcs,
        }
    }
}
