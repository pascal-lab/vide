//! Name join over `FileFacts` plus an optional generated-unit map.

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use smol_str::SmolStr;
use vfs::FileId;

use crate::{
    db::DesignGraphDb,
    unit::{InstantiationRole, UnitId, UnitKind, UnitOrigin},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitMeta {
    pub kind: UnitKind,
    pub origin: UnitOrigin,
    pub header_fingerprint: u64,
}

/// One file's generated units, valid only for a specific artifact fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileUnits {
    pub fingerprint: u64,
    pub ids: Box<[UnitId]>,
}

/// Generated units recorded from a paid artifact. No ranges.
///
/// Entries are keyed by `(FileId, compilation_unit_snapshot.fingerprint)`.
/// A FileId-only lookup cannot return a stale set: [`Self::ids_for`] and
/// [`Self::retain_current`] treat a fingerprint mismatch as a miss.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedUnits {
    pub by_file: FxHashMap<FileId, GeneratedFileUnits>,
    pub meta: FxHashMap<UnitId, UnitMeta>,
}

impl GeneratedUnits {
    pub fn contains_file(&self, file: FileId) -> bool {
        self.by_file.contains_key(&file)
    }

    pub fn ids_for(&self, file: FileId) -> &[UnitId] {
        self.by_file.get(&file).map(|entry| entry.ids.as_ref()).unwrap_or(&[])
    }

    /// Keep only entries for which `is_current(file, stored_fingerprint)` is
    /// true. Returns the files that were dropped.
    pub fn retain_current(&mut self, is_current: impl Fn(FileId, u64) -> bool) -> Vec<FileId> {
        let mut dropped = Vec::new();
        self.by_file.retain(|&file, entry| {
            if is_current(file, entry.fingerprint) {
                true
            } else {
                for id in entry.ids.iter() {
                    self.meta.remove(id);
                }
                dropped.push(file);
                false
            }
        });
        dropped
    }

    /// Replace one file's generated ids. Returns whether the stored set
    /// changed.
    pub fn replace_file(
        &mut self,
        file: FileId,
        fingerprint: u64,
        ids: Box<[UnitId]>,
        meta: FxHashMap<UnitId, UnitMeta>,
    ) -> bool {
        let previous = self.by_file.get(&file);
        if previous.is_some_and(|entry| {
            entry.fingerprint == fingerprint && entry.ids.as_ref() == ids.as_ref()
        }) {
            return false;
        }
        if let Some(old) = self.by_file.insert(file, GeneratedFileUnits { fingerprint, ids }) {
            for id in old.ids.iter() {
                self.meta.remove(id);
            }
        }
        self.meta.extend(meta);
        true
    }
}

/// A lookup result that preserves the difference between no match, one
/// logical definition, and several competing definitions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resolution<T> {
    Unresolved,
    Unique(T),
    Ambiguous(SmallVec<[T; 2]>),
}

impl<T> Resolution<T> {
    pub fn candidates(&self) -> &[T] {
        match self {
            Self::Unresolved => &[],
            Self::Unique(value) => std::slice::from_ref(value),
            Self::Ambiguous(candidates) => candidates,
        }
    }

    pub fn into_candidates(self) -> SmallVec<[T; 2]> {
        match self {
            Self::Unresolved => SmallVec::new(),
            Self::Unique(value) => {
                let mut candidates = SmallVec::new();
                candidates.push(value);
                candidates
            }
            Self::Ambiguous(candidates) => candidates,
        }
    }

    pub fn into_vec(self) -> SmallVec<[T; 2]> {
        self.into_candidates()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.candidates().iter()
    }

    pub fn is_unresolved(&self) -> bool {
        matches!(self, Self::Unresolved)
    }

    pub fn or_else(self, fallback: impl FnOnce() -> Self) -> Self {
        if self.is_unresolved() { fallback() } else { self }
    }
}

impl<T: Clone> Resolution<T> {
    pub fn unique(&self) -> Option<T> {
        match self {
            Self::Unique(item) => Some(item.clone()),
            Self::Ambiguous(_) | Self::Unresolved => None,
        }
    }

    /// Resolves children without allowing child existence to disambiguate an
    /// ambiguous parent.
    pub fn and_then<U: Eq>(&self, mut resolve: impl FnMut(T) -> Resolution<U>) -> Resolution<U> {
        let children = Resolution::from_candidates(
            self.iter().cloned().flat_map(|candidate| resolve(candidate).into_candidates()),
        );
        match (self, children) {
            (Self::Ambiguous(_), Resolution::Unique(_)) => Resolution::Unresolved,
            (_, children) => children,
        }
    }
}

impl<T> From<T> for Resolution<T> {
    fn from(value: T) -> Self {
        Self::Unique(value)
    }
}

impl<T: Eq> Resolution<T> {
    pub fn from_candidates(candidates: impl IntoIterator<Item = T>) -> Self {
        let mut unique = SmallVec::<[T; 2]>::new();
        for candidate in candidates {
            if !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }
        match unique.len() {
            0 => Self::Unresolved,
            1 => Self::Unique(unique.pop().expect("candidate length was checked")),
            _ => Self::Ambiguous(unique),
        }
    }

    pub fn map<U: Eq>(self, map: impl FnMut(T) -> U) -> Resolution<U> {
        Resolution::from_candidates(self.into_candidates().into_iter().map(map))
    }
}

/// Structure product: name → `UnitId`. Stores no source ranges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitCatalog {
    by_name: FxHashMap<SmolStr, SmallVec<[UnitId; 2]>>,
    meta: FxHashMap<UnitId, UnitMeta>,
    module_names: Vec<SmolStr>,
}

impl UnitCatalog {
    /// Join already-extracted per-file facts. Callers that can run `file_facts`
    /// in parallel should do that and pass the results here.
    pub fn from_decls<'a>(
        decls: impl IntoIterator<Item = &'a crate::DeclIndex>,
        generated: &GeneratedUnits,
    ) -> Self {
        let mut graph = Self::default();
        for decls in decls {
            for unit in decls.units.iter() {
                graph.insert(
                    unit.id.clone(),
                    UnitMeta {
                        kind: unit.id.kind,
                        origin: unit.origin,
                        header_fingerprint: unit.header_fingerprint,
                    },
                );
            }
        }
        for (id, meta) in generated.meta.iter() {
            graph.insert(id.clone(), meta.clone());
        }
        graph.rebuild_module_names();
        graph
    }

    /// Replace one file's source and generated units. Other files stay.
    /// Returns whether the node set for `file` changed.
    pub fn upsert_file(
        &mut self,
        file: FileId,
        facts: &crate::FileFacts,
        generated: &GeneratedUnits,
    ) -> bool {
        let mut next = Vec::new();
        for unit in facts.units.iter() {
            debug_assert_eq!(unit.id.file, file);
            next.push((
                unit.id.clone(),
                UnitMeta {
                    kind: unit.id.kind,
                    origin: unit.origin,
                    header_fingerprint: unit.header_fingerprint,
                },
            ));
        }
        for id in generated.ids_for(file) {
            if let Some(meta) = generated.meta.get(id) {
                next.push((id.clone(), meta.clone()));
            }
        }
        let mut previous: Vec<_> = self
            .meta
            .iter()
            .filter(|(id, _)| id.file == file)
            .map(|(id, meta)| (id.clone(), meta.clone()))
            .collect();
        previous.sort_by(|left, right| {
            left.0.ordinal.cmp(&right.0.ordinal).then_with(|| left.0.name.cmp(&right.0.name))
        });
        next.sort_by(|left, right| {
            left.0.ordinal.cmp(&right.0.ordinal).then_with(|| left.0.name.cmp(&right.0.name))
        });
        if previous == next {
            return false;
        }
        self.remove_file(file);
        for (id, meta) in next {
            self.insert(id, meta);
        }
        self.rebuild_module_names();
        true
    }

    /// Drop every node owned by `file`. Returns whether anything was removed.
    pub fn remove_file(&mut self, file: FileId) -> bool {
        let ids: Vec<_> = self.meta.keys().filter(|id| id.file == file).cloned().collect();
        if ids.is_empty() {
            return false;
        }
        for id in ids {
            self.meta.remove(&id);
            if let Some(list) = self.by_name.get_mut(&id.name) {
                list.retain(|existing| existing != &id);
                if list.is_empty() {
                    self.by_name.remove(&id.name);
                }
            }
        }
        self.rebuild_module_names();
        true
    }

    fn rebuild_module_names(&mut self) {
        self.module_names = self
            .by_name
            .iter()
            .filter(|(_, ids)| ids.iter().any(|id| id.kind.is_hierarchy_target()))
            .map(|(name, _)| name.clone())
            .collect();
        self.module_names.sort();
        self.module_names.dedup();
    }

    /// `file_facts` come from salsa; `generated` comes from the product store.
    pub fn fold(db: &dyn DesignGraphDb, generated: &GeneratedUnits) -> Self {
        let facts: Vec<_> = db
            .files()
            .iter()
            .copied()
            .filter(|&file_id| db.file_kind(file_id).is_semantic_compilation_unit())
            .map(|file_id| db.file_facts(file_id))
            .collect();
        let decls: Vec<_> = facts.iter().map(|facts| facts.decls()).collect();
        Self::from_decls(decls.iter(), generated)
    }

    pub(crate) fn insert(&mut self, id: UnitId, meta: UnitMeta) {
        self.by_name.entry(id.name.clone()).or_default().push(id.clone());
        self.meta.insert(id, meta);
    }

    pub fn modules_named(&self, name: &str) -> Resolution<UnitId> {
        self.named(name, |id| id.kind.is_hierarchy_target())
    }

    pub fn type_units_named(&self, name: &str) -> Resolution<UnitId> {
        self.named(name, |_| true)
    }

    pub fn packages_named(&self, name: &str) -> Resolution<UnitId> {
        self.named(name, |id| id.kind.is_package())
    }

    pub fn packages(&self) -> impl Iterator<Item = UnitId> + '_ {
        self.meta.keys().filter(|id| id.kind.is_package()).cloned()
    }

    pub fn module_names(&self) -> &[SmolStr] {
        &self.module_names
    }

    pub fn contains(&self, id: &UnitId) -> bool {
        self.meta.contains_key(id)
    }

    pub fn origin(&self, id: &UnitId) -> Option<UnitOrigin> {
        self.meta.get(id).map(|meta| meta.origin)
    }

    pub fn node_count(&self) -> usize {
        self.meta.len()
    }

    pub fn candidates(&self, name: &str, role: InstantiationRole) -> SmallVec<[UnitId; 2]> {
        let matches = match role {
            InstantiationRole::Hierarchy => UnitKind::is_hierarchy_target,
            InstantiationRole::Checker => |kind: UnitKind| matches!(kind, UnitKind::Checker),
        };
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .filter(|id| matches(id.kind))
            .cloned()
            .collect()
    }

    fn named(&self, name: &str, pred: impl Fn(&UnitId) -> bool) -> Resolution<UnitId> {
        Resolution::from_candidates(
            self.by_name.get(name).into_iter().flatten().filter(|id| pred(id)).cloned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use rustc_hash::FxHashMap;
    use smol_str::SmolStr;
    use vfs::FileId;

    use super::{GeneratedUnits, UnitMeta};
    use crate::unit::{UnitId, UnitKind, UnitOrigin};

    const FILE: FileId = FileId::from_raw(1);

    fn id(name: &str, ordinal: u32) -> UnitId {
        UnitId { file: FILE, name: SmolStr::new(name), kind: UnitKind::Module, ordinal }
    }

    fn generated_meta(id: &UnitId) -> UnitMeta {
        UnitMeta { kind: id.kind, origin: UnitOrigin::Generated, header_fingerprint: 0 }
    }

    #[test]
    fn replace_file_is_noop_when_ids_match() {
        let mut generated = GeneratedUnits::default();
        let unit = id("foo", 0);
        let mut meta = FxHashMap::default();
        meta.insert(unit.clone(), generated_meta(&unit));
        assert!(generated.replace_file(FILE, 1, Box::new([unit.clone()]), meta.clone()));
        assert!(!generated.replace_file(FILE, 1, Box::new([unit]), meta));
    }

    #[test]
    fn retain_current_drops_a_mismatched_fingerprint() {
        let mut generated = GeneratedUnits::default();
        let unit = id("foo", 0);
        let mut meta = FxHashMap::default();
        meta.insert(unit.clone(), generated_meta(&unit));
        assert!(generated.replace_file(FILE, 1, Box::new([unit.clone()]), meta));
        let dropped = generated.retain_current(|_, fingerprint| fingerprint == 2);
        assert_eq!(dropped, vec![FILE]);
        assert!(generated.ids_for(FILE).is_empty());
        assert!(!generated.meta.contains_key(&unit));
    }

    #[test]
    fn replace_file_drops_previous_meta() {
        let mut generated = GeneratedUnits::default();
        let old = id("foo", 0);
        let new = id("bar", 0);
        let mut old_meta = FxHashMap::default();
        old_meta.insert(old.clone(), generated_meta(&old));
        assert!(generated.replace_file(FILE, 1, Box::new([old.clone()]), old_meta));
        let mut new_meta = FxHashMap::default();
        new_meta.insert(new.clone(), generated_meta(&new));
        assert!(generated.replace_file(FILE, 2, Box::new([new.clone()]), new_meta));
        assert!(!generated.meta.contains_key(&old));
        assert!(generated.meta.contains_key(&new));
    }

    #[test]
    fn from_file_facts_joins_source_units_and_generated() {
        let unit = crate::unit::UnitNode {
            id: id("src", 0),
            origin: UnitOrigin::Source,
            name_range: None,
            header_range: None,
            header_fingerprint: 1,
        };
        let facts =
            crate::FileFacts { units: Box::new([unit.clone()]), ..crate::FileFacts::default() };
        let generated_id = id("gen", 0);
        let mut generated = GeneratedUnits::default();
        let mut meta = FxHashMap::default();
        meta.insert(generated_id.clone(), generated_meta(&generated_id));
        generated.replace_file(FILE, 1, Box::new([generated_id.clone()]), meta);

        let decls = facts.decls();
        let graph = super::UnitCatalog::from_decls(std::iter::once(&decls), &generated);
        assert!(graph.contains(&unit.id));
        assert!(graph.contains(&generated_id));
        assert_eq!(graph.node_count(), 2);
    }

    #[test]
    fn upsert_file_replaces_one_file_and_keeps_the_other() {
        let other = FileId::from_raw(2);
        let keep =
            UnitId { file: other, name: SmolStr::new("keep"), kind: UnitKind::Module, ordinal: 0 };
        let mut graph = super::UnitCatalog::default();
        graph.insert(keep.clone(), generated_meta(&keep));
        graph.rebuild_module_names();

        let first = crate::unit::UnitNode {
            id: id("first", 0),
            origin: UnitOrigin::Source,
            name_range: None,
            header_range: None,
            header_fingerprint: 1,
        };
        let facts =
            crate::FileFacts { units: Box::new([first.clone()]), ..crate::FileFacts::default() };
        assert!(graph.upsert_file(FILE, &facts, &GeneratedUnits::default()));
        assert!(graph.contains(&keep));
        assert!(graph.contains(&first.id));

        let second = crate::unit::UnitNode {
            id: id("second", 0),
            origin: UnitOrigin::Source,
            name_range: None,
            header_range: None,
            header_fingerprint: 2,
        };
        let facts =
            crate::FileFacts { units: Box::new([second.clone()]), ..crate::FileFacts::default() };
        assert!(graph.upsert_file(FILE, &facts, &GeneratedUnits::default()));
        assert!(graph.contains(&keep));
        assert!(graph.contains(&second.id));
        assert!(!graph.contains(&first.id));
        assert!(!graph.upsert_file(FILE, &facts, &GeneratedUnits::default()));
    }

    #[test]
    fn remove_file_drops_only_that_file() {
        let other = FileId::from_raw(2);
        let keep =
            UnitId { file: other, name: SmolStr::new("keep"), kind: UnitKind::Module, ordinal: 0 };
        let mut graph = super::UnitCatalog::default();
        graph.insert(id("gone", 0), generated_meta(&id("gone", 0)));
        graph.insert(keep.clone(), generated_meta(&keep));
        graph.rebuild_module_names();
        assert!(graph.remove_file(FILE));
        assert!(graph.contains(&keep));
        assert!(!graph.contains(&id("gone", 0)));
        assert!(!graph.remove_file(FILE));
    }
}
