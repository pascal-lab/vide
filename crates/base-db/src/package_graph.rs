use itertools::Itertools;
use la_arena::{Arena, Idx};
use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use std::{
    fmt,
    ops::{self, Index},
};
use vfs::vfs::FileId;

#[derive(Default)]
pub struct PackageGraph {
    arena: Arena<PackageInfo>,
}

impl PackageGraph {
    pub fn add_package(&mut self, package: PackageInfo) -> PackageId {
        self.arena.alloc(package)
    }

    pub fn add_dep(
        &mut self,
        from: PackageId,
        dep: PackageDependency,
    ) -> Result<(), CyclicDependenciesError> {
        if let Some(path) = self.find_path(from, dep.package_id) {
            // &&* is used to make self immutable to make borrow checker happy
            return Err(CyclicDependenciesError { graph: &&*self, path });
        }

        self.arena[from].add_dep(dep);
        Ok(())
    }

    fn find_path(&self, from: PackageId, to: PackageId) -> Option<Vec<PackageId>> {
        fn dfs(
            packs: &PackageGraph,
            visited: &mut FxHashSet<PackageId>,
            cur: PackageId,
            to: PackageId,
        ) -> Option<Vec<PackageId>> {
            if cur == to {
                return Some(vec![to]);
            }

            visited.insert(cur);

            for dep in packs[cur].deps.iter() {
                let nxt = dep.package_id;
                if visited.contains(&nxt) {
                    continue;
                }
                if let Some(mut path) = dfs(packs, visited, nxt, to) {
                    path.push(nxt);
                    return Some(path);
                }
            }

            None
        }

        if let Some(mut path) = dfs(&self, &mut FxHashSet::default(), from, to) {
            path.reverse();
            assert!(path.first().is_some_and(|first| *first == from));
            assert!(path.last().is_some_and(|last| *last == to));
            return Some(path);
        }

        None
    }

    pub fn iter(&self) -> impl Iterator<Item = PackageId> + '_ {
        self.arena.iter().map(|(idx, _)| idx)
    }

    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }
}

impl fmt::Debug for PackageGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.arena.iter().map(|(id, data)| (u32::from(id.into_raw()), data)))
            .finish()
    }
}

pub type PackageId = Idx<PackageInfo>;

impl ops::Index<PackageId> for PackageGraph {
    type Output = PackageInfo;

    fn index(&self, idx: PackageId) -> &Self::Output {
        &self.arena[idx]
    }
}

#[derive(Debug)]
pub struct PackageName(SmolStr);

impl ops::Deref for PackageName {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub struct PackageInfo {
    pub root_file_id: FileId,
    pub name: PackageName,
    pub deps: Vec<PackageDependency>,
}

impl PackageInfo {
    fn add_dep(&mut self, dep: PackageDependency) {
        self.deps.push(dep)
    }
}

#[derive(Debug)]
pub struct PackageDependency {
    pub package_id: PackageId,
    pub name: PackageName,
}

#[derive(Debug)]
pub struct CyclicDependenciesError<'a> {
    graph: &'a PackageGraph,
    path: Vec<PackageId>,
}

impl<'a> CyclicDependenciesError<'a> {
    fn get_pack_info(&self, idx: Idx<PackageInfo>) -> &'a PackageInfo {
        self.graph.arena.index(idx)
    }
}

impl<'a> fmt::Display for CyclicDependenciesError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self
            .path
            .iter()
            .map(|idx| self.get_pack_info(*idx).name.to_string())
            .collect_vec()
            .join(" -> ");
        write!(f, "{}", path)
    }
}
