use std::ops::Deref;

use la_arena::{ArenaMap, Idx};
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
pub(crate) use smol_str::SmolStr;
use syntax::{SyntaxKind, SyntaxTree, ast::AstNode};
use triomphe::Arc;
use utils::{
    get::{Get, GetRef},
    text_edit::TextRange,
};

use crate::{ast_id_map::SourceAstId, db::HirDefDb};

pub trait LoweredData: std::fmt::Debug + Eq {
    type SourceMap: std::fmt::Debug + Eq;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoweringDiagnosticKind {
    InvalidSyntax,
    UnsupportedSyntax,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringDiagnostic {
    pub kind: LoweringDiagnosticKind,
    pub syntax_kind: SyntaxKind,
    pub source: Option<SourceAstId>,
    pub range: Option<TextRange>,
    pub message: SmolStr,
}

/// Position-free HIR and its canonical source identities.
///
/// Revision-local pointers and ranges live in `AstIdMap` and
/// `SourceProjection`. Source-facing methods query those modules explicitly so
/// semantic lowering remains independent of whole-file position changes.
#[derive(Debug, PartialEq, Eq)]
pub struct Lowered<T: LoweredData> {
    file_id: HirFileId,
    data: Arc<T>,
    source_map: Arc<T::SourceMap>,
    diagnostics: Arc<[LoweringDiagnostic]>,
}

impl<T: LoweredData> Lowered<T> {
    pub fn new(file_id: HirFileId, data: T, source_map: T::SourceMap) -> Self {
        Self::new_with_diagnostics(file_id, data, source_map, Vec::new())
    }

    pub(crate) fn new_with_diagnostics(
        file_id: HirFileId,
        data: T,
        source_map: T::SourceMap,
        diagnostics: Vec<LoweringDiagnostic>,
    ) -> Self {
        Self {
            file_id,
            data: Arc::new(data),
            source_map: Arc::new(source_map),
            diagnostics: diagnostics.into(),
        }
    }

    pub fn file_id(&self) -> HirFileId {
        self.file_id
    }

    pub fn data(&self) -> Arc<T> {
        Arc::clone(&self.data)
    }

    pub fn data_ref(&self) -> &T {
        &self.data
    }

    pub fn source_map(&self) -> &T::SourceMap {
        &self.source_map
    }

    pub fn diagnostics(&self, db: &dyn HirDefDb) -> Vec<LoweringDiagnostic> {
        let projection = db.source_projection(self.file_id);
        self.diagnostics
            .iter()
            .cloned()
            .map(|mut diagnostic| {
                diagnostic.range = diagnostic.range.or_else(|| {
                    diagnostic
                        .source
                        .and_then(|source| projection.origin(source))
                        .and_then(|origin| origin.full_range())
                });
                diagnostic
            })
            .collect()
    }

    pub(crate) fn raw_diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn source_map_arc(&self) -> Arc<T::SourceMap> {
        Arc::clone(&self.source_map)
    }

    pub fn source<Id>(&self, id: Id) -> <T::SourceMap as Get<Id>>::Output
    where
        T::SourceMap: Get<Id>,
    {
        self.source_map.get(id)
    }

    pub fn source_range<Id>(&self, db: &dyn HirDefDb, id: Id) -> Option<TextRange>
    where
        T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    {
        db.source_projection(self.file_id).origin(self.source_map.get(id)?)?.full_range()
    }

    pub fn source_name_range<Id>(&self, db: &dyn HirDefDb, id: Id) -> Option<TextRange>
    where
        T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    {
        db.source_projection(self.file_id).origin(self.source_map.get(id)?)?.focus_range()
    }

    pub fn source_name_or_full_range<Id>(&self, db: &dyn HirDefDb, id: Id) -> Option<TextRange>
    where
        T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    {
        db.source_projection(self.file_id).origin(self.source_map.get(id)?)?.focus_or_full_range()
    }

    pub fn source_info<Id>(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo>
    where
        T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    {
        SourceInfo::from_origin(
            db.source_projection(self.file_id).origin(self.source_map.get(id)?)?,
        )
    }

    pub fn named_source_info<Id>(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo>
    where
        T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    {
        self.source_info(db, id)
    }

    pub fn hir_id<Id>(&self, source: SourceAstId) -> Option<Id>
    where
        T::SourceMap: Get<SourceAstId, Output = Option<Id>>,
    {
        self.source_map.get(source)
    }

    pub fn hir_id_for_node<Id>(
        &self,
        db: &dyn HirDefDb,
        tree: &SyntaxTree,
        node: syntax::SyntaxNode<'_>,
    ) -> Option<Id>
    where
        T::SourceMap: Get<SourceAstId, Output = Option<Id>>,
    {
        let source = db.ast_id_map(self.file_id).id_of_node_in_tree(tree, node)?;
        self.hir_id(source)
    }

    pub fn get<Id>(&self, id: Id) -> &<T as GetRef<Id>>::Output
    where
        T: GetRef<Id>,
    {
        self.data.get(id)
    }
}

impl<T: LoweredData> Deref for Lowered<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: LoweredData> AsRef<T> for Lowered<T> {
    fn as_ref(&self) -> &T {
        &self.data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceInfo {
    kind: Option<SyntaxKind>,
    full_range: TextRange,
    focus_range: Option<TextRange>,
}

impl SourceInfo {
    pub(crate) fn from_origin(origin: crate::source_projection::SourceOrigin) -> Option<Self> {
        Some(Self {
            kind: origin.kind(),
            full_range: origin.full_range()?,
            focus_range: origin.focus_range(),
        })
    }

    pub fn from_ranges(full_range: TextRange, focus_range: Option<TextRange>) -> Self {
        Self { kind: None, full_range, focus_range }
    }

    pub fn from_parts(
        kind: Option<SyntaxKind>,
        full_range: TextRange,
        focus_range: Option<TextRange>,
    ) -> Self {
        Self { kind, full_range, focus_range }
    }

    pub fn kind(self) -> Option<SyntaxKind> {
        self.kind
    }

    pub fn full_range(self) -> TextRange {
        self.full_range
    }

    pub fn focus_range(self) -> Option<TextRange> {
        self.focus_range
    }

    pub fn focus_or_full_range(self) -> TextRange {
        self.focus_range.unwrap_or(self.full_range)
    }
}

pub trait HirLookup<Id> {
    type Hir;

    fn hir(&self, id: Id) -> &Self::Hir;
}

impl<T, Id> HirLookup<Id> for Lowered<T>
where
    T: LoweredData + GetRef<Id>,
{
    type Hir = <T as GetRef<Id>>::Output;

    fn hir(&self, id: Id) -> &Self::Hir {
        self.data.get(id)
    }
}

pub trait SourceLookup<Id> {
    fn source_info(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo>;
}

impl<T, Id> SourceLookup<Id> for Lowered<T>
where
    T: LoweredData,
    T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
{
    fn source_info(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo> {
        Lowered::source_info(self, db, id)
    }
}

pub trait NamedSourceLookup<Id> {
    fn named_source_info(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo>;
}

impl<T, Id> NamedSourceLookup<Id> for Lowered<T>
where
    T: LoweredData,
    T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
{
    fn named_source_info(&self, db: &dyn HirDefDb, id: Id) -> Option<SourceInfo> {
        Lowered::source_info(self, db, id)
    }
}

pub trait AstLookup<'a, Id, Node: AstNode<'a>> {
    fn ast(&self, db: &dyn HirDefDb, id: Id, tree: &'a SyntaxTree) -> Option<Node>;
}

impl<'a, T, Id, Node> AstLookup<'a, Id, Node> for Lowered<T>
where
    T: LoweredData,
    T::SourceMap: Get<Id, Output = Option<SourceAstId>>,
    Node: AstNode<'a>,
{
    fn ast(&self, db: &dyn HirDefDb, id: Id, tree: &'a SyntaxTree) -> Option<Node> {
        let source = self.source_map.get(id)?;
        Node::cast(db.ast_id_map(self.file_id).node(source, tree)?)
    }
}

/// Bidirectional relation between one HIR arena and canonical source AST ids.
///
/// The map deliberately contains no syntax pointer, range, kind, name token,
/// or file id. Those are revision-local projection data owned by `AstIdMap`
/// and `SourceProjection` and keyed by the same `SourceAstId`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceMap<Hir> {
    src2hir: FxHashMap<SourceAstId, Idx<Hir>>,
    hir2src: ArenaMap<Idx<Hir>, SourceAstId>,
}

impl<Hir> SourceMap<Hir> {
    pub fn insert(&mut self, source: SourceAstId, idx: Idx<Hir>) {
        self.assert_hir_slot(idx, source);
        self.src2hir.insert(source, idx);
        self.hir2src.insert(idx, source);
    }

    pub fn insert_alias(&mut self, source: SourceAstId, idx: Idx<Hir>) {
        self.assert_target(idx);
        self.src2hir.insert(source, idx);
    }

    pub fn insert_preferred_alias(&mut self, source: SourceAstId, idx: Idx<Hir>) {
        self.insert_alias(source, idx);
        self.hir2src.insert(idx, source);
    }

    fn assert_target(&self, idx: Idx<Hir>) {
        assert!(self.hir2src.get(idx).is_some(), "source map alias target has no canonical source");
    }

    fn assert_hir_slot(&self, idx: Idx<Hir>, source: SourceAstId) {
        if let Some(existing) = self.hir2src.get(idx) {
            assert_eq!(
                *existing, source,
                "source map HIR value already has another canonical source"
            );
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.src2hir.shrink_to_fit();
        self.hir2src.shrink_to_fit();
    }

    pub fn iter(&self) -> impl Iterator<Item = (Idx<Hir>, SourceAstId)> + '_ {
        self.hir2src.iter().map(|(id, source)| (id, *source))
    }

    pub fn ranges<'a>(
        &'a self,
        projection: &'a crate::source_projection::SourceProjection,
    ) -> impl Iterator<Item = TextRange> + 'a {
        self.hir2src.iter().filter_map(|(_, source)| projection.origin(*source)?.full_range())
    }

    pub fn named_ranges<'a>(
        &'a self,
        projection: &'a crate::source_projection::SourceProjection,
    ) -> impl Iterator<Item = (Idx<Hir>, TextRange, Option<TextRange>)> + 'a {
        self.hir2src.iter().filter_map(|(id, source)| {
            let origin = projection.origin(*source)?;
            Some((id, origin.full_range()?, origin.focus_range()))
        })
    }

    #[inline]
    pub fn src_to_hir(&self, source: SourceAstId) -> Option<Idx<Hir>> {
        self.src2hir.get(&source).copied()
    }

    #[inline]
    pub fn hir_to_src(&self, idx: Idx<Hir>) -> Option<SourceAstId> {
        self.hir2src.get(idx).copied()
    }
}

impl<Hir> Get<SourceAstId> for SourceMap<Hir> {
    type Output = Option<Idx<Hir>>;

    fn get(&self, source: SourceAstId) -> Self::Output {
        self.src_to_hir(source)
    }
}

impl<Hir> Get<Idx<Hir>> for SourceMap<Hir> {
    type Output = Option<SourceAstId>;

    fn get(&self, idx: Idx<Hir>) -> Self::Output {
        self.hir_to_src(idx)
    }
}

impl<Hir> Default for SourceMap<Hir> {
    fn default() -> Self {
        Self { src2hir: FxHashMap::default(), hir2src: ArenaMap::default() }
    }
}

#[cfg(test)]
mod tests {

    use la_arena::Arena;

    use super::{SourceAstId, SourceMap};

    #[test]
    fn aliases_do_not_change_the_canonical_source() {
        let mut arena = Arena::default();
        let hir = arena.alloc(());
        let primary = SourceAstId::from_raw(1);
        let alias = SourceAstId::from_raw(10);
        let mut map = SourceMap::default();

        map.insert(primary, hir);
        map.insert_alias(alias, hir);
        assert_eq!(map.src_to_hir(alias), Some(hir));
        assert_eq!(map.hir_to_src(hir), Some(primary));

        map.insert_preferred_alias(alias, hir);
        assert_eq!(map.hir_to_src(hir), Some(alias));
    }

    #[test]
    #[should_panic(expected = "source map HIR value already has another canonical source")]
    fn a_hir_value_cannot_have_two_canonical_sources() {
        let mut arena = Arena::default();
        let hir = arena.alloc(());
        let mut map = SourceMap::default();
        map.insert(SourceAstId::from_raw(0), hir);
        map.insert(SourceAstId::from_raw(10), hir);
    }
}
