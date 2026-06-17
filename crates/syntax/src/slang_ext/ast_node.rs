use slang::ast::AstNode;

use crate::ptr::SyntaxNodePtr;

pub trait AstNodeExt {
    fn to_ptr(&self) -> SyntaxNodePtr;
}

impl<'a, T> AstNodeExt for T
where
    T: AstNode<'a>,
{
    #[inline]
    fn to_ptr(&self) -> SyntaxNodePtr {
        SyntaxNodePtr::from_node(self.syntax())
    }
}
