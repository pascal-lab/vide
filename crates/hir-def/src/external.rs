use la_arena::Idx;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    alloc_with_source_entry,
    lower::{BodyStore, LoweringCtx, LoweringStore},
    subroutine::{Subroutine, lower_subroutine_prototype},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ExternInterfaceMethod {
    pub method: Subroutine,
    pub fork_join: bool,
}

pub type ExternInterfaceMethodId = Idx<ExternInterfaceMethod>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_extern_interface_method(
        &mut self,
        declaration: ast::ExternInterfaceMethod,
    ) -> Option<ExternInterfaceMethodId> {
        let prototype = declaration.prototype();
        let Some(keyword) = prototype.keyword() else {
            self.report_invalid(
                declaration.syntax(),
                "extern interface method is missing its subroutine kind",
            );
            return None;
        };
        let is_task = match keyword.kind() {
            TokenKind::TASK_KEYWORD => true,
            TokenKind::FUNCTION_KEYWORD => false,
            _ => {
                self.report_invalid(
                    declaration.syntax(),
                    "extern interface method has an invalid subroutine kind",
                );
                return None;
            }
        };
        let ast_ids = self.ast_ids.clone();
        let tree = self.tree.clone();
        let Some(method) = lower_subroutine_prototype(
            prototype,
            is_task,
            false,
            |ty| self.lower_data_ty(ty),
            &ast_ids,
            &tree,
        ) else {
            self.report_invalid(
                declaration.syntax(),
                "extern interface method has an invalid prototype",
            );
            return None;
        };
        let fork_join = match declaration.fork_join() {
            None => false,
            Some(token) if token.kind() == TokenKind::FORK_JOIN_KEYWORD => true,
            Some(_) => {
                self.report_invalid(
                    declaration.syntax(),
                    "extern interface method has an invalid forkjoin qualifier",
                );
                return None;
            }
        };
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.extern_interface_methods,
            &mut sources.extern_interface_method_srcs,
            ExternInterfaceMethod { method, fork_join },
            source,
        ))
    }
}
