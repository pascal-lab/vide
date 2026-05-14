pub mod aggregate;
pub mod block;
pub mod declaration;
pub mod expr;
pub mod file;
pub mod literal;
pub mod module;
pub mod proc;
pub mod stmt;
pub mod subroutine;
pub mod ty;
pub mod typedef;

use la_arena::{Arena, Idx, RawIdx};
use smol_str::{SmolStr, ToSmolStr};
use syntax::{SyntaxToken, ast};

pub type Ident = SmolStr;

pub const DEFAULT_NAME: SmolStr = SmolStr::new_static("unnamed");

#[inline]
pub fn lower_ident(ident: Option<SyntaxToken>) -> Option<Ident> {
    Some(ident?.value_text().to_smolstr())
}

// If the ident is empty, return None, which may represent a missing identifier.
#[inline]
pub fn lower_ident_opt(ident: Option<SyntaxToken>) -> Option<Ident> {
    let ident = lower_ident(ident)?;
    if ident.is_empty() { None } else { Some(ident) }
}

#[inline]
pub(crate) fn lower_named_label_opt(label: Option<ast::NamedLabel>) -> Option<Ident> {
    let ident = lower_ident(label?.name())?;
    if ident.is_empty() { None } else { Some(ident) }
}

macro alloc_idx_and_src($hir:expr => $arena:expr, $ast:expr => $src_map:expr $(,)?) {{
    let idx = $arena.alloc($hir.into());
    let src = $ast.into();
    $src_map.insert(src, idx);
    idx
}}

trait HirData<T> {
    fn nxt_idx(&self) -> Idx<T>;
}

impl<T> HirData<T> for Arena<T> {
    #[inline]
    fn nxt_idx(&self) -> Idx<T> {
        Idx::from_raw(RawIdx::from(self.len() as u32))
    }
}
