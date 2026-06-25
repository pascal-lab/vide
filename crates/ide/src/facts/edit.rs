use utils::line_index::TextRange;

use crate::{
    FilePosition,
    db::root_db::RootDb,
    rename::{self, RecursiveRenameInfo, RenameCollisionInfo, RenameConfig, RenameResult},
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
    PrepareRename(TextRange),
    Rename(SourceChange),
    RenameExpansionInfo(RecursiveRenameInfo),
    RenameConflictInfo(RenameCollisionInfo),
}

impl EditPlan {
    pub(crate) fn into_prepare_rename(self) -> TextRange {
        match self {
            EditPlan::PrepareRename(range) => range,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_source_change(self) -> SourceChange {
        match self {
            EditPlan::Rename(change) => change,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_rename_expansion_info(self) -> RecursiveRenameInfo {
        match self {
            EditPlan::RenameExpansionInfo(info) => info,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }

    pub(crate) fn into_rename_conflict_info(self) -> RenameCollisionInfo {
        match self {
            EditPlan::RenameConflictInfo(info) => info,
            _ => unreachable!("edit request and edit plan variant should match"),
        }
    }
}

pub(crate) fn edit_plan(db: &RootDb, request: EditRequest<'_>) -> RenameResult<EditPlan> {
    match request {
        EditRequest::PrepareRename { position, config } => {
            rename::prepare_rename(db, position, config).map(EditPlan::PrepareRename)
        }
        EditRequest::Rename { position, config, new_name } => {
            rename::rename(db, position, config, new_name).map(EditPlan::Rename)
        }
        EditRequest::RenameExpansionInfo { position, config } => {
            rename::rename_expansion_info(db, position, config).map(EditPlan::RenameExpansionInfo)
        }
        EditRequest::ExpandedRename { position, config, new_name } => {
            rename::expanded_rename(db, position, config, new_name).map(EditPlan::Rename)
        }
        EditRequest::RenameConflictInfo { position, config, new_name, recursive } => {
            rename::rename_conflict_info(db, position, config, new_name, recursive)
                .map(EditPlan::RenameConflictInfo)
        }
    }
}
