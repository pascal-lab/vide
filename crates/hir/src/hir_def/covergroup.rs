use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use crate::{
    hir_def::{Ident, lower_ident_opt, lower_named_label_opt},
    source_map::{FromSourceAst, IsNamedSrc, IsSrc, SourceAst, root_token_in},
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CovergroupSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CoverpointSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CrossSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

macro_rules! impl_cover_src {
    ($src:ty) => {
        impl IsSrc for $src {
            #[inline]
            fn kind(&self) -> SyntaxKind {
                self.node.kind()
            }

            #[inline]
            fn range(&self) -> TextRange {
                self.node.range()
            }
        }

        impl IsNamedSrc for $src {
            #[inline]
            fn name_kind(&self) -> Option<TokenKind> {
                self.name.map(|name| name.kind())
            }

            #[inline]
            fn name_range(&self) -> Option<TextRange> {
                self.name.map(|name| name.range())
            }
        }
    };
}

impl_cover_src!(CovergroupSrc);
impl_cover_src!(CoverpointSrc);
impl_cover_src!(CrossSrc);

impl<'a> FromSourceAst<'a, ast::CovergroupDeclaration<'a>> for CovergroupSrc {
    fn from_source_ast(covergroup: SourceAst<ast::CovergroupDeclaration<'a>>) -> Self {
        let covergroup = covergroup.into_inner();
        let syntax = covergroup.syntax();
        let name = covergroup
            .name()
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&covergroup), name }
    }
}

impl<'a> FromSourceAst<'a, ast::Coverpoint<'a>> for CoverpointSrc {
    fn from_source_ast(coverpoint: SourceAst<ast::Coverpoint<'a>>) -> Self {
        let coverpoint = coverpoint.into_inner();
        let syntax = coverpoint.syntax();
        let name = coverpoint
            .label()
            .and_then(|label| label.name())
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&coverpoint), name }
    }
}

impl<'a> FromSourceAst<'a, ast::CoverCross<'a>> for CrossSrc {
    fn from_source_ast(cross: SourceAst<ast::CoverCross<'a>>) -> Self {
        let cross = cross.into_inner();
        let syntax = cross.syntax();
        let name = cross
            .label()
            .and_then(|label| label.name())
            .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token));
        Self { node: AstNodeExt::to_ptr(&cross), name }
    }
}

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
