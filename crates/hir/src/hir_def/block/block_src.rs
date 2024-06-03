use syntax::{
    ast::ptr::{self, AstNodePtr},
    syntax_kind,
};
use utils::impl_from;

use crate::container::InFile;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalBlockSrc {
    SeqBlock(ptr::SeqBlockPtr),
    ParBlock(ptr::ParBlockPtr),
}

impl AstNodePtr for LocalBlockSrc {
    fn can_cast(kind_id: syntax_kind::SyntaxKindId) -> bool {
        kind_id == syntax_kind::SEQ_BLOCK || kind_id == syntax_kind::PAR_BLOCK
    }
    fn cast(syntax: syntax::SyntaxNodePtr) -> Option<LocalBlockSrc> {
        match syntax.kind_id() {
            syntax_kind::SEQ_BLOCK => Some(ptr::SeqBlockPtr::cast(syntax)?.into()),
            syntax_kind::PAR_BLOCK => Some(ptr::ParBlockPtr::cast(syntax)?.into()),
            _ => None,
        }
    }
    fn syntax(&self) -> &syntax::SyntaxNodePtr {
        match self {
            LocalBlockSrc::SeqBlock(ptr) => ptr.syntax(),
            LocalBlockSrc::ParBlock(ptr) => ptr.syntax(),
        }
    }
}

impl_from!(ptr::SeqBlockPtr as SeqBlock, ptr::ParBlockPtr as ParBlock for LocalBlockSrc);

pub type BlockSrc = InFile<LocalBlockSrc>;
