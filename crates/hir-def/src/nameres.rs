//! One cached scope map per compilation unit (file).
//!
//! `DefMap` is the single authority for name-scope data. The nine
//! `*_scope` salsa queries delegate here (see `scope.rs`), and name
//! resolution (`pathres.rs`) walks scopes from this map instead of
//! issuing one query per scope hop.
//!
//! Scopes are enumerated eagerly per file: the file's own scope plus every
//! scope reachable from it (modules, generate blocks, blocks, subroutines,
//! checkers, covergroups, clocking blocks). Generate blocks are interned at
//! lowering time, so eager enumeration is complete and acyclic.

use la_arena::Arena;
use preproc_expand::file::HirFileId;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;

use crate::{
    block::{Block, BlockInfo},
    container::{
        FileOrModule, InFileOrModule, InModule, ScopeId, SubroutineParent, SubroutineScope,
    },
    db::HirDefDb,
    module::{ModuleId, generate::GenerateItem},
    scope::{
        build_block_scope, build_checker_scope, build_clocking_block_scope, build_covergroup_scope,
        build_file_scope, build_generate_block_scope, build_module_scope, build_subroutine_scope,
    },
    stmt::{Stmt, StmtKind},
    symbol::NameScope,
};

pub(crate) fn def_map_query(db: &dyn HirDefDb, file_id: HirFileId) -> Arc<DefMap> {
    Arc::new(DefMap::for_file(db, file_id))
}

/// The name scopes of one compilation unit (file), prebuilt and cached.
#[derive(Debug, PartialEq, Eq)]
pub struct DefMap {
    scopes: FxHashMap<ScopeId, Arc<NameScope>>,
}

impl DefMap {
    pub(crate) fn for_file(db: &dyn HirDefDb, file_id: HirFileId) -> Self {
        let mut scopes = FxHashMap::default();
        let mut pending = Vec::new();
        let mut visited = FxHashSet::default();

        // Seed: the file scope plus every module owned by this file.
        let file_scope_id = ScopeId::File(file_id);
        scopes.insert(file_scope_id, Arc::new(build_file_scope(db, file_id)));
        visited.insert(file_scope_id);
        pending.push(file_scope_id);

        let hir_file = db.hir_file_with_source_map(file_id);
        for (local_id, _) in hir_file.modules.iter() {
            let module_id = ModuleId::new(file_id, local_id);
            push_scope(db, file_id, module_id.into(), &mut scopes, &mut pending, &mut visited);
        }

        // BFS over reachable scopes. Generate blocks are interned during
        // lowering, so following every child reference terminates.
        while let Some(scope_id) = pending.pop() {
            for child in child_scope_ids(db, scope_id) {
                if child.file_id(db) != file_id {
                    continue;
                }
                push_scope(db, file_id, child, &mut scopes, &mut pending, &mut visited);
            }
        }

        DefMap { scopes }
    }

    /// The scope for `scope_id`, or a freshly built scope as a correctness
    /// net if enumeration missed it (a bug we fail soft on).
    pub(crate) fn scope(&self, db: &dyn HirDefDb, scope_id: ScopeId) -> Arc<NameScope> {
        self.scopes.get(&scope_id).cloned().unwrap_or_else(|| Arc::new(build_scope(db, scope_id)))
    }
}

fn push_scope(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    scope_id: ScopeId,
    scopes: &mut FxHashMap<ScopeId, Arc<NameScope>>,
    pending: &mut Vec<ScopeId>,
    visited: &mut FxHashSet<ScopeId>,
) {
    debug_assert_eq!(scope_id.file_id(db), file_id, "scope must belong to its unit's file");
    if !visited.insert(scope_id) {
        return;
    }
    scopes.insert(scope_id, Arc::new(build_scope(db, scope_id)));
    pending.push(scope_id);
}

fn build_scope(db: &dyn HirDefDb, scope_id: ScopeId) -> NameScope {
    match scope_id {
        ScopeId::File(file_id) => build_file_scope(db, file_id),
        ScopeId::Module(module_id) => build_module_scope(db, module_id),
        ScopeId::ClockingBlock(clocking_block_id) => {
            build_clocking_block_scope(db, clocking_block_id)
        }
        ScopeId::Checker(checker_id) => build_checker_scope(db, checker_id),
        ScopeId::Covergroup(covergroup_id) => build_covergroup_scope(db, covergroup_id),
        ScopeId::GenerateBlock(generate_block_id) => {
            build_generate_block_scope(db, generate_block_id)
        }
        ScopeId::Block(block_id) => build_block_scope(db, block_id),
        ScopeId::Subroutine(subroutine_id) => build_subroutine_scope(db, subroutine_id),
    }
}

/// The scopes directly contained in `scope_id`, for DefMap enumeration.
fn child_scope_ids(db: &dyn HirDefDb, scope_id: ScopeId) -> Vec<ScopeId> {
    let mut children = Vec::new();
    match scope_id {
        ScopeId::File(file_id) => {
            let hir_file = db.hir_file_with_source_map(file_id);
            for (local_id, _) in hir_file.modules.iter() {
                children.push(ModuleId::new(file_id, local_id).into());
            }
            for (checker_id, _) in hir_file.checkers.iter() {
                children.push(ScopeId::Checker(InFileOrModule::new(
                    FileOrModule::File(file_id),
                    checker_id,
                )));
            }
            for (covergroup_id, _) in hir_file.covergroups.iter() {
                children.push(ScopeId::Covergroup(InFileOrModule::new(
                    FileOrModule::File(file_id),
                    covergroup_id,
                )));
            }
            collect_block_ids(db, &hir_file.stmts, &mut children);
        }
        ScopeId::Module(module_id) => {
            let module = db.module_with_source_map(module_id);
            for (local_id, _) in module.subroutines.iter() {
                children.push(ScopeId::Subroutine(SubroutineScope::new(
                    SubroutineParent::Module(module_id),
                    local_id,
                )));
            }
            for (checker_id, _) in module.checkers.iter() {
                children.push(ScopeId::Checker(InFileOrModule::new(
                    FileOrModule::Module(module_id),
                    checker_id,
                )));
            }
            for (covergroup_id, _) in module.covergroups.iter() {
                children.push(ScopeId::Covergroup(InFileOrModule::new(
                    FileOrModule::Module(module_id),
                    covergroup_id,
                )));
            }
            for (clocking_block_id, _) in module.clocking_blocks.iter() {
                children.push(ScopeId::ClockingBlock(InModule::new(module_id, clocking_block_id)));
            }
            for (region_id, region) in module.generate_regions.iter() {
                for item in &region.items {
                    if let GenerateItem::GenerateBlockId(generate_block_id) = *item {
                        children.push(generate_block_id.into());
                    }
                }
                let _ = region_id;
            }
            collect_block_ids(db, &module.stmts, &mut children);
        }
        ScopeId::GenerateBlock(generate_block_id) => {
            let generate_block = db.generate_block_with_source_map(generate_block_id);
            for (local_id, _) in generate_block.subroutines.iter() {
                children.push(ScopeId::Subroutine(SubroutineScope::new(
                    SubroutineParent::GenerateBlock(generate_block_id),
                    local_id,
                )));
            }
            for item in &generate_block.items {
                if let crate::module::generate::GenerateBlockItem::GenerateBlockId(child_id) = *item
                {
                    children.push(child_id.into());
                }
            }
            collect_block_ids(db, &generate_block.stmts, &mut children);
        }
        ScopeId::Block(block_id) => {
            let block = db.block_with_source_map(block_id);
            collect_block_ids(db, &block.stmts, &mut children);
        }
        ScopeId::Subroutine(subroutine_id) => {
            let subroutine = db.subroutine_with_source_map(subroutine_id);
            collect_block_ids(db, &subroutine.stmts, &mut children);
        }
        ScopeId::ClockingBlock(_) | ScopeId::Checker(_) | ScopeId::Covergroup(_) => {}
    }
    children
}

/// Collect named blocks (and, transitively, their nested named blocks) from a
/// statement arena. Proc bodies live in the owner's statement arena, so
/// walking the owner arena covers blocks inside procedural blocks too.
fn collect_block_ids(db: &dyn HirDefDb, stmts: &Arena<Stmt>, out: &mut Vec<ScopeId>) {
    for (_, stmt) in stmts.iter() {
        if let StmtKind::Block(BlockInfo { block_id, .. }) = &stmt.kind {
            let block_id = *block_id;
            out.push(block_id.into());
            let block: &Block = &db.block_with_source_map(block_id);
            collect_block_ids(db, &block.stmts, out);
        }
    }
}
