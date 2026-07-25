//! High-level `Semantics` entry point.
//!
//! Wraps `hir_semantics::Semantics` and translates its implementation-level
//! returns (`DefId`, `ModuleId`, source-map iteration) into facade types
//! (`Definition`, `Module`, `Source`). IDE code calls these instead of
//! reaching into `hir-def` / `preproc-expand`.

use hir_def::{container::InFile, db::HirDefDb, def_id::DefId};
use hir_semantics::semantics::Semantics as InnerSemantics;
use syntax::ast::AstNode;
use utils::text_edit::TextSize;
use vfs::FileId;

use crate::{File, Module, ParsedFile, Source};

/// The high-level HIR entry point for IDE features.
///
/// Construct one per request from the shared database; it caches source-to-def
/// resolution for its lifetime. `DB` is whatever concrete salsa database the
/// host uses (e.g. `RootDb`); it must provide HIR definition queries.
pub struct Semantics<'db, DB: HirDefDb> {
    inner: InnerSemantics<'db, DB>,
}

impl<'db, DB: HirDefDb> Semantics<'db, DB> {
    pub fn new(db: &'db DB) -> Self {
        Self { inner: InnerSemantics::new(db) }
    }

    /// Facade-internal access to the underlying adapter. Other modules of this
    /// crate use it for source-to-def resolution; IDE code cannot.
    pub(crate) fn inner(&self) -> &InnerSemantics<'db, DB> {
        &self.inner
    }

    /// Parses a real source file into a [`ParsedFile`]. Replaces direct
    /// `hir_semantics::Semantics::parse_file` calls; no implementation type leaks.
    pub fn parse_file(&self, file_id: FileId) -> ParsedFile {
        ParsedFile::new(self.inner.parse_file(file_id))
    }

    /// Finds the smallest AST node of type `N` covering `offset` within
    /// `node`. Pure syntax traversal; the facade forwards to the underlying
    /// adapter so IDE code does not name `hir_semantics::Semantics`.
    pub fn find_node_at_offset<'a, N: AstNode<'a>>(
        &self,
        node: syntax::SyntaxNode<'a>,
        offset: TextSize,
    ) -> Option<N> {
        self.inner.find_node_at_offset(node, offset)
    }

    /// Returns every module declaration in `file`, paired with its source
    /// location, in declaration order. Anonymous modules are included; callers
    /// that only care about named modules filter on [`Module::name`].
    ///
    /// Replaces the manual `Lowered<HirFile>` source-map iteration that IDE
    /// code previously performed inline.
    pub fn module_declarations(&self, file: File) -> Vec<(Source, Module)> {
        let lowered = self.inner.db.hir_file_with_source_map(file.into_inner());
        let mut out = Vec::new();
        for (local, _info) in lowered.modules.iter() {
            let Some(range) = lowered.source_range(local) else { continue };
            out.push((Source::new(file, range), Module::new(file, local)));
        }
        out
    }

    /// Returns the module declaration whose source range starts at `offset` in
    /// `file`, if any.
    ///
    /// Replaces the manual "find the module at this offset, build a `ModuleId`,
    /// intern a `DefId`" dance that IDE code previously performed step by step.
    pub fn module_at_offset(&self, file: File, offset: TextSize) -> Option<Module> {
        let lowered = self.inner.db.hir_file_with_source_map(file.into_inner());
        lowered
            .modules
            .iter()
            .find(|(local, _)| {
                lowered.source_range(*local).is_some_and(|r| r.start() == offset)
            })
            .map(|(local, _)| Module::new(file, local))
    }

    /// Builds the [`Definition`] for a given [`Module`]. Replaces the manual
    /// `ModuleId::new` + `DefId::new` interning dance.
    pub fn definition_of(&self, module: Module) -> Definition {
        let loc = InFile::new(module.file().into_inner(), module.local_id());
        Definition::new(DefId::new(self.inner.db, loc))
    }
}

/// A resolved definition, independent of the arena identifier that backs it.
///
/// Wraps a `DefId` (the interned definition origin) without exposing it. IDE
/// code obtains a `Definition` from [`Semantics`] and queries it through the
/// facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Definition {
    def_id: DefId,
}

impl Definition {
    pub(crate) fn new(def_id: DefId) -> Self {
        Self { def_id }
    }

    /// Facade-internal access to the backing identifier. Other modules of this
    /// crate (e.g. the references engine) use this; IDE code cannot.
    pub(crate) fn def_id(self) -> DefId {
        self.def_id
    }
}