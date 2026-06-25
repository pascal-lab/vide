use hir::{container::InFile, semantics::Semantics};
use smol_str::SmolStr;
use utils::{line_index::TextRange, uniq_vec::UniqVec};

use crate::{
    FilePosition, FileRange,
    db::root_db::RootDb,
    definitions::{Definition, DefinitionOrigin},
    rename::{
        self, RecursiveRenameInfo, RenameCollisionInfo, RenameConfig, RenameResult,
        ResolvedRenameTarget,
    },
    source_change::SourceChange,
};

pub(crate) enum EditRequest<'a> {
    PrepareRename {
        position: FilePosition,
        config: RenameConfig,
    },
    Rename {
        position: FilePosition,
        config: RenameConfig,
        new_name: &'a str,
    },
    RenameExpansionInfo {
        position: FilePosition,
        config: RenameConfig,
    },
    ExpandedRename {
        position: FilePosition,
        config: RenameConfig,
        new_name: &'a str,
    },
    RenameConflictInfo {
        position: FilePosition,
        config: RenameConfig,
        new_name: &'a str,
        recursive: bool,
    },
}

pub(crate) enum EditPlan {
    PrepareRename(RenamePreparePlan),
    Rename(RenameEditPlan),
    RenameExpansionInfo(RenameExpansionPlan),
    RenameConflictInfo(RenameConflictPlan),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameTargetPlan {
    pub(crate) range: TextRange,
    pub(crate) selected_symbols: Vec<DefinitionOrigin>,
    pub(crate) related_symbols: Vec<DefinitionOrigin>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameSymbolPlan {
    pub(crate) symbols: Vec<DefinitionOrigin>,
    pub(crate) definition_ranges: Vec<FileRange>,
    pub(crate) reference_ranges: Vec<FileRange>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameSymbolEditPlan {
    pub(crate) symbol: RenameSymbolPlan,
    pub(crate) change: SourceChange,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenamePreparePlan {
    pub(crate) target: RenameTargetPlan,
    pub(crate) editable_range: TextRange,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameEditPlan {
    pub(crate) target: RenameTargetPlan,
    pub(crate) recursive: bool,
    pub(crate) new_name: String,
    pub(crate) symbols: Vec<RenameSymbolEditPlan>,
    pub(crate) change: SourceChange,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameExpansionPlan {
    pub(crate) target: RenameTargetPlan,
    pub(crate) symbols: Vec<RenameSymbolPlan>,
    pub(crate) info: RecursiveRenameInfo,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RenameConflictPlan {
    pub(crate) target: RenameTargetPlan,
    pub(crate) symbols: Vec<DefinitionOrigin>,
    pub(crate) info: RenameCollisionInfo,
}

impl EditPlan {
    pub(crate) fn into_prepare_rename(self) -> TextRange {
        match self {
            EditPlan::PrepareRename(plan) => plan.editable_range,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_source_change(self) -> SourceChange {
        match self {
            EditPlan::Rename(plan) => plan.change,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_rename_expansion_info(self) -> RecursiveRenameInfo {
        match self {
            EditPlan::RenameExpansionInfo(plan) => plan.info,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_rename_conflict_info(self) -> RenameCollisionInfo {
        match self {
            EditPlan::RenameConflictInfo(plan) => plan.info,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }
}

pub(crate) fn edit_plan(db: &RootDb, request: EditRequest<'_>) -> RenameResult<EditPlan> {
    let facts = EditFacts { db };
    match request {
        EditRequest::PrepareRename { position, config } => {
            facts.prepare_rename(position, config).map(EditPlan::PrepareRename)
        }
        EditRequest::Rename { position, config, new_name } => {
            facts.rename(position, config, new_name).map(EditPlan::Rename)
        }
        EditRequest::RenameExpansionInfo { position, config } => {
            facts.rename_expansion_info(position, config).map(EditPlan::RenameExpansionInfo)
        }
        EditRequest::ExpandedRename { position, config, new_name } => {
            facts.expanded_rename(position, config, new_name).map(EditPlan::Rename)
        }
        EditRequest::RenameConflictInfo { position, config, new_name, recursive } => facts
            .rename_conflict_info(position, config, new_name, recursive)
            .map(EditPlan::RenameConflictInfo),
    }
}

struct EditFacts<'db> {
    db: &'db RootDb,
}

impl<'db> EditFacts<'db> {
    fn prepare_rename(
        &self,
        position @ FilePosition { file_id, .. }: FilePosition,
        config: RenameConfig,
    ) -> RenameResult<RenamePreparePlan> {
        let sema = Semantics::new(self.db);
        let resolved = rename::resolve_rename_target(&sema, position)?;
        let _ = config.references_config(self.db, &resolved.selected_def, file_id)?;
        Ok(RenamePreparePlan {
            editable_range: resolved.range,
            target: rename_target_plan(&resolved),
        })
    }

    fn rename(
        &self,
        position @ FilePosition { file_id, .. }: FilePosition,
        config: RenameConfig,
        new_name: &str,
    ) -> RenameResult<RenameEditPlan> {
        let sema = Semantics::new(self.db);
        let resolved = rename::resolve_rename_target(&sema, position)?;
        let refs = rename::references_for_definition(
            self.db,
            &sema,
            file_id,
            &config,
            &resolved.selected_def,
        )?;
        let symbol =
            self.symbol_edit_plan(&sema, &resolved.selected_def, new_name, None, &refs, &[])?;
        Ok(RenameEditPlan {
            target: rename_target_plan(&resolved),
            recursive: false,
            new_name: new_name.to_owned(),
            change: symbol.change.clone(),
            symbols: vec![symbol],
        })
    }

    fn rename_expansion_info(
        &self,
        position: FilePosition,
        config: RenameConfig,
    ) -> RenameResult<RenameExpansionPlan> {
        let sema = Semantics::new(self.db);
        let resolved = rename::resolve_rename_target(&sema, position)?;
        let target = rename_target_plan(&resolved);
        let targets = rename::recursive_rename_targets(
            self.db,
            &sema,
            position.file_id,
            &config,
            resolved.targets,
        )?;
        let symbols = targets
            .iter()
            .map(|target| self.symbol_plan(&target.def, &target.refs))
            .collect::<Vec<_>>();

        Ok(RenameExpansionPlan {
            target,
            info: RecursiveRenameInfo { additional_symbols: targets.len().saturating_sub(1) },
            symbols,
        })
    }

    fn expanded_rename(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
    ) -> RenameResult<RenameEditPlan> {
        let sema = Semantics::new(self.db);
        let resolved = rename::resolve_rename_target(&sema, position)?;
        let target = rename_target_plan(&resolved);
        let targets = rename::recursive_rename_targets(
            self.db,
            &sema,
            position.file_id,
            &config,
            resolved.targets,
        )?;
        let rename_targets = rename_target_index(&targets);
        let mut change = SourceChange::default();
        let mut symbols = Vec::new();

        for target in &targets {
            let symbol = self.symbol_edit_plan(
                &sema,
                &target.def,
                new_name,
                Some(&rename_targets),
                &target.refs,
                &target.same_name_refs,
            )?;
            merge_source_change(&mut change, symbol.change.clone())?;
            symbols.push(symbol);
        }

        Ok(RenameEditPlan {
            target,
            recursive: true,
            new_name: new_name.to_owned(),
            symbols,
            change,
        })
    }

    fn rename_conflict_info(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
        recursive: bool,
    ) -> RenameResult<RenameConflictPlan> {
        let sema = Semantics::new(self.db);
        let resolved = rename::resolve_rename_target(&sema, position)?;
        let target = rename_target_plan(&resolved);
        let defs: Vec<Definition> = if recursive {
            rename::recursive_rename_targets(
                self.db,
                &sema,
                position.file_id,
                &config,
                resolved.targets,
            )?
            .into_iter()
            .map(|target| target.def)
            .collect()
        } else {
            vec![resolved.selected_def]
        };

        let new_name = SmolStr::new(new_name);
        let mut target_index = UniqVec::<(), DefinitionOrigin>::default();
        for target in &defs {
            target_index.push(target.origins(), ());
        }
        let mut conflicts = UniqVec::<Definition, DefinitionOrigin>::default();
        for collision in defs.iter().flat_map(|target| target.origins()).filter_map(|origin| {
            sema.resolve_name(origin.container_id(self.db), &new_name).map(Definition::from)
        }) {
            if collision.origins().iter().any(|origin| target_index.contains(origin)) {
                continue;
            }
            conflicts.push(collision.origins(), collision);
        }

        Ok(RenameConflictPlan {
            target,
            symbols: definitions_symbols(&defs),
            info: RenameCollisionInfo { conflicts: conflicts.len() },
        })
    }

    fn symbol_edit_plan(
        &self,
        sema: &Semantics<'_, RootDb>,
        def: &Definition,
        new_name: &str,
        rename_targets: Option<&UniqVec<(), DefinitionOrigin>>,
        refs: &rename::ReferenceSearchResult,
        same_name_refs: &[rename::SameNameConnectionRef],
    ) -> RenameResult<RenameSymbolEditPlan> {
        let symbol = self.symbol_plan(def, refs);
        let change = rename::rename_definition_with_refs(
            self.db,
            sema,
            def,
            new_name,
            rename_targets,
            refs,
            same_name_refs,
        )?;
        Ok(RenameSymbolEditPlan { symbol, change })
    }

    fn symbol_plan(
        &self,
        def: &Definition,
        refs: &rename::ReferenceSearchResult,
    ) -> RenameSymbolPlan {
        RenameSymbolPlan {
            symbols: definition_symbols(def),
            definition_ranges: definition_ranges(self.db, def),
            reference_ranges: reference_ranges(refs),
        }
    }
}

fn rename_target_plan(resolved: &ResolvedRenameTarget) -> RenameTargetPlan {
    RenameTargetPlan {
        range: resolved.range,
        selected_symbols: definition_symbols(&resolved.selected_def),
        related_symbols: definitions_symbols(&resolved.targets),
    }
}

fn rename_target_index(targets: &[rename::RecursiveRenameTarget]) -> UniqVec<(), DefinitionOrigin> {
    let mut index = UniqVec::<(), DefinitionOrigin>::default();
    for target in targets {
        index.push(target.def.origins(), ());
    }
    index
}

fn definitions_symbols(defs: &[Definition]) -> Vec<DefinitionOrigin> {
    let mut symbols = UniqVec::<DefinitionOrigin, DefinitionOrigin>::default();
    for def in defs {
        for symbol in def.origins() {
            symbols.push_unique(symbol);
        }
    }
    symbols.into_vec()
}

fn definition_symbols(def: &Definition) -> Vec<DefinitionOrigin> {
    definitions_symbols(std::slice::from_ref(def))
}

fn definition_ranges(db: &RootDb, def: &Definition) -> Vec<FileRange> {
    let mut ranges = UniqVec::<FileRange, FileRange>::default();
    for origin in def.origins() {
        let Some(InFile { file_id, value: range }) = origin.name_range(db) else {
            continue;
        };
        ranges.push_unique(FileRange { file_id: file_id.file_id(), range });
    }
    ranges.into_vec()
}

fn reference_ranges(refs: &rename::ReferenceSearchResult) -> Vec<FileRange> {
    let mut ranges = UniqVec::<FileRange, FileRange>::default();
    for (&file_id, refs) in refs {
        for reference in refs {
            ranges.push_unique(FileRange { file_id, range: reference.range() });
        }
    }
    ranges.into_vec()
}

fn merge_source_change(target: &mut SourceChange, change: SourceChange) -> RenameResult<()> {
    for (file_id, edit) in change.text_edits {
        target
            .insert_text_edit(file_id, edit)
            .map_err(|_| rename::RenameError::OverlappingEdits)?;
    }
    Ok(())
}
