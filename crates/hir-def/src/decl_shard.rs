//! Per-file L0 declaration shard.
//!
//! Extracted from a throwaway unexpanded parse. The C++ syntax tree is not
//! stored: salsa memos this compact value, not a `SyntaxTree`.

use preproc_expand::file::HirFileId;
use smol_str::SmolStr;
use syntax::TokenKind;
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::{ast_id_map::SyntaxFileId, db::HirDefDb};

mod extract;

/// What a compilation-unit declaration is, without an `OwnerId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclRole {
    Module,
    Interface,
    Package,
    Program,
    Checker,
    Covergroup,
    Typedef,
    Param,
    Net,
    Var,
    Subroutine,
    Other,
}

impl DeclRole {
    pub fn is_design_unit(self) -> bool {
        matches!(
            self,
            Self::Module
                | Self::Interface
                | Self::Package
                | Self::Program
                | Self::Checker
                | Self::Covergroup
        )
    }

    pub fn is_instantiable_module(self) -> bool {
        matches!(self, Self::Module | Self::Interface | Self::Program)
    }
}

/// One CU-scope declaration recorded from the source text of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decl {
    pub name: SmolStr,
    pub role: DeclRole,
    pub ordinal: u32,
    pub header_fingerprint: u64,
    /// Name token in this file's display coordinates. Absent when the extract
    /// tree could not assign a single-buffer range.
    pub name_range: Option<TextRange>,
}

/// One name-like token, unresolved.
///
/// `emitted` is the preprocessor-trace index when the extract tree assigned
/// one. Macro-expanded tokens share display ranges, so later recovery on the
/// authoritative parse needs this identity when the two traces agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    pub name: SmolStr,
    pub kind: TokenKind,
    pub range: TextRange,
    pub emitted: Option<u32>,
}

/// `import p::x` / `import p::*`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSpec {
    pub package: SmolStr,
    pub item: Option<SmolStr>,
}

/// Compact L0 slice of one file. No syntax tree, no interned owner.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDeclShard {
    pub decls: Box<[Decl]>,
    pub mentions: Box<[Mention]>,
    pub imports: Box<[ImportSpec]>,
    pub preprocessor_independent: bool,
    pub has_compilation_unit_locals: bool,
}

impl FileDeclShard {
    pub fn mentions_name(&self, name: &str) -> bool {
        self.mentions.iter().any(|mention| mention.name == name)
    }

    pub fn has_compilation_unit_locals(&self) -> bool {
        self.has_compilation_unit_locals
    }

    /// Design-unit whose recorded name token covers `offset`.
    pub fn design_unit_at(&self, offset: utils::line_index::TextSize) -> Option<&Decl> {
        self.decls.iter().find(|decl| {
            decl.role.is_design_unit()
                && decl.name_range.is_some_and(|range| range.contains(offset))
        })
    }
}

#[salsa::tracked(lru = 256, returns(clone))]
pub fn file_decl_shard(db: &dyn HirDefDb, file: SyntaxFileId) -> Arc<FileDeclShard> {
    let HirFileId::File(file_id) = file.hir_file(db) else {
        return Arc::new(FileDeclShard::default());
    };
    Arc::new(extract::collect(db, file_id))
}

pub(crate) fn set_decl_shard_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    file_decl_shard::set_lru_capacity(db, capacity);
}

impl dyn HirDefDb + '_ {
    pub fn file_decl_shard(&self, file_id: FileId) -> Arc<FileDeclShard> {
        file_decl_shard(self, self.syntax_file(HirFileId::File(file_id)))
    }
}
