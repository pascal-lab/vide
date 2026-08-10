use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};

use crate::{
    alloc_with_source_entry,
    expr::ExprId,
    lower::{BodyStore, LoweringCtx, LoweringStore},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetAlias {
    pub nets: SmallVec<[ExprId; 4]>,
}

pub type NetAliasId = Idx<NetAlias>;

impl LoweringCtx<BodyStore<'_>> {
    pub(crate) fn lower_net_alias(&mut self, alias: ast::NetAlias) -> Option<NetAliasId> {
        if alias.keyword().map(|token| token.kind()) != Some(TokenKind::ALIAS_KEYWORD) {
            self.report_invalid(alias.syntax(), "net alias is missing its alias keyword");
            return None;
        }
        let nets: SmallVec<[_; 4]> =
            alias.nets().children().map(|net| self.lower_expr(net)).collect();
        if nets.len() < 2 {
            self.report_invalid(alias.syntax(), "net alias must contain at least two nets");
            return None;
        }
        let source = self.source_id(alias.syntax());
        let (body, sources) = self.store.body();
        Some(alloc_with_source_entry(
            &mut body.net_aliases,
            &mut sources.net_alias_srcs,
            NetAlias { nets },
            source,
        ))
    }
}
