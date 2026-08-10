use la_arena::Idx;
use smallvec::SmallVec;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryDecl {
    pub name: Option<Ident>,
    pub file_paths: SmallVec<[Ident; 4]>,
    pub include_dirs: SmallVec<[Ident; 2]>,
}

pub type LibraryDeclId = Idx<LibraryDecl>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryInclude {
    pub file_path: Option<Ident>,
}

pub type LibraryIncludeId = Idx<LibraryInclude>;
