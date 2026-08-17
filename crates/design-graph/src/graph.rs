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

/// Generated units recorded by the IDE from a paid artifact. No ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedUnits {
    pub by_file: FxHashMap<FileId, Box<[UnitId]>>,
    pub meta: FxHashMap<UnitId, UnitMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphResolution<T> {
    Unique(T),
    Ambiguous(SmallVec<[T; 2]>),
    Unresolved,
}

impl<T> GraphResolution<T> {
    pub fn from_candidates(mut candidates: SmallVec<[T; 2]>) -> Self {
        match candidates.len() {
            0 => Self::Unresolved,
            1 => Self::Unique(candidates.remove(0)),
            _ => Self::Ambiguous(candidates),
        }
    }

    pub fn into_vec(self) -> SmallVec<[T; 1]> {
        match self {
            Self::Unique(item) => {
                let mut items = SmallVec::new();
                items.push(item);
                items
            }
            Self::Ambiguous(items) => items.into_iter().collect(),
            Self::Unresolved => SmallVec::new(),
        }
    }
}

/// Structure product: name → `UnitId`. Stores no source ranges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesignGraph {
    by_name: FxHashMap<SmolStr, SmallVec<[UnitId; 2]>>,
    meta: FxHashMap<UnitId, UnitMeta>,
    module_names: Vec<SmolStr>,
}

impl DesignGraph {
    /// `file_facts` come from salsa; `generated` comes from the product store.
    pub fn fold(db: &dyn DesignGraphDb, generated: &GeneratedUnits) -> Self {
        let mut graph = Self::default();
        for file_id in db
            .files()
            .iter()
            .copied()
            .filter(|&file_id| db.file_kind(file_id).is_semantic_compilation_unit())
        {
            for unit in db.file_facts(file_id).units.iter() {
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
        graph.module_names = graph
            .by_name
            .iter()
            .filter(|(_, ids)| ids.iter().any(|id| id.kind.is_hierarchy_target()))
            .map(|(name, _)| name.clone())
            .collect();
        graph.module_names.sort();
        graph.module_names.dedup();
        graph
    }

    fn insert(&mut self, id: UnitId, meta: UnitMeta) {
        self.by_name.entry(id.name.clone()).or_default().push(id.clone());
        self.meta.insert(id, meta);
    }

    pub fn modules_named(&self, name: &str) -> GraphResolution<UnitId> {
        self.named(name, |id| id.kind.is_hierarchy_target())
    }

    pub fn type_units_named(&self, name: &str) -> GraphResolution<UnitId> {
        self.named(name, |_| true)
    }

    pub fn packages_named(&self, name: &str) -> GraphResolution<UnitId> {
        self.named(name, |id| id.kind.is_package())
    }

    pub fn top_level_modules_named(&self, name: &str) -> GraphResolution<UnitId> {
        self.modules_named(name)
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

    pub fn candidates(&self, name: &str, role: InstantiationRole) -> SmallVec<[UnitId; 1]> {
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

    fn named(&self, name: &str, pred: impl Fn(&UnitId) -> bool) -> GraphResolution<UnitId> {
        let candidates =
            self.by_name.get(name).into_iter().flatten().filter(|id| pred(id)).cloned().collect();
        GraphResolution::from_candidates(candidates)
    }
}
