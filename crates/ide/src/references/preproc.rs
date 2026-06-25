use hir::preproc::{
    MacroDefinition, MacroParamDefinition, MacroReferenceIndexStatus, macro_param_references,
    macro_references,
};
use itertools::Itertools;
use vfs::FileId;

use super::{
    ReferenceCategory, References, ReferencesConfig, ReferencesPartialReason, ReferencesStatus,
};
use crate::{db::root_db::RootDb, facts::target::PreprocMacroTarget, navigation_target::NavTarget};

pub(super) fn render_preproc_references_target(
    db: &RootDb,
    file_id: FileId,
    target: PreprocMacroTarget,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    match target {
        PreprocMacroTarget::ParamDefinition(definition) => {
            Some(vec![macro_param_references_for_definition(db, file_id, definition, config)?])
        }
        PreprocMacroTarget::ParamReference(resolution) => {
            let definitions = resolution.definitions;
            if definitions.is_empty() {
                return None;
            }
            definitions
                .into_iter()
                .map(|definition| {
                    macro_param_references_for_definition(db, file_id, definition, config)
                })
                .collect()
        }
        PreprocMacroTarget::Definition(definition) => {
            Some(vec![macro_references_for_definition(db, file_id, definition, config)?])
        }
        PreprocMacroTarget::Reference(resolution) => {
            let definitions = resolution.definitions;
            if definitions.is_empty() {
                return None;
            }
            definitions
                .into_iter()
                .map(|definition| macro_references_for_definition(db, file_id, definition, config))
                .collect()
        }
    }
}

fn macro_param_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition: MacroParamDefinition,
    config: &ReferencesConfig,
) -> Option<References> {
    let refs = macro_param_references(db, file_id, &definition)
        .ok()?
        .references
        .into_iter()
        .filter(|usage| {
            config.search_scope.as_ref().is_none_or(|scope| {
                scope.range_for_file(usage.file_id).is_some_and(|range| {
                    range.is_none_or(|range| range.intersect(usage.range).is_some())
                })
            })
        })
        .into_group_map_by(|usage| usage.file_id)
        .into_iter()
        .map(|(file_id, usages)| {
            (
                file_id,
                usages
                    .into_iter()
                    .map(|usage| (usage.range, ReferenceCategory::empty()))
                    .collect_vec(),
            )
        })
        .collect();
    Some(References {
        def: Some(vec![macro_param_nav_target(definition)]),
        refs,
        status: ReferencesStatus::Complete,
    })
}

fn macro_references_for_definition(
    db: &RootDb,
    file_id: FileId,
    definition: MacroDefinition,
    config: &ReferencesConfig,
) -> Option<References> {
    let references = macro_references(db, file_id, &definition).ok()?;
    let status = references_status_from_macro_index(references.status);
    let refs = references
        .references
        .into_iter()
        .filter(|usage| {
            config.search_scope.as_ref().is_none_or(|scope| {
                scope.range_for_file(usage.file_id).is_some_and(|range| {
                    range.is_none_or(|range| range.intersect(usage.range).is_some())
                })
            })
        })
        .into_group_map_by(|usage| usage.file_id)
        .into_iter()
        .map(|(file_id, usages)| {
            (
                file_id,
                usages
                    .into_iter()
                    .map(|usage| (usage.range, ReferenceCategory::empty()))
                    .collect_vec(),
            )
        })
        .collect();
    Some(References { def: Some(vec![macro_nav_target(definition)]), refs, status })
}

fn references_status_from_macro_index(status: MacroReferenceIndexStatus) -> ReferencesStatus {
    match status {
        MacroReferenceIndexStatus::Complete => ReferencesStatus::Complete,
        MacroReferenceIndexStatus::Partial { issue_count } => ReferencesStatus::Partial {
            reason: ReferencesPartialReason::PreprocMacroIndex,
            issue_count,
        },
    }
}

fn macro_param_nav_target(definition: MacroParamDefinition) -> NavTarget {
    NavTarget {
        file_id: definition.macro_definition.file_id,
        full_range: definition.range,
        focus_range: Some(definition.range),
        name: Some(definition.name),
        kind: None,
        container_name: Some(definition.macro_definition.name),
        description: Some("macro parameter".to_owned()),
    }
}

fn macro_nav_target(definition: MacroDefinition) -> NavTarget {
    NavTarget {
        file_id: definition.file_id,
        full_range: definition.name_range,
        focus_range: Some(definition.name_range),
        name: Some(definition.name),
        kind: None,
        container_name: None,
        description: Some("macro definition".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use hir::preproc::MacroReferenceIndexStatus;

    use super::*;

    #[test]
    fn macro_reference_index_status_maps_to_reference_status() {
        assert_eq!(
            references_status_from_macro_index(MacroReferenceIndexStatus::Complete),
            ReferencesStatus::Complete
        );

        let status = references_status_from_macro_index(MacroReferenceIndexStatus::Partial {
            issue_count: 1,
        });

        assert_eq!(
            status,
            ReferencesStatus::Partial {
                reason: ReferencesPartialReason::PreprocMacroIndex,
                issue_count: 1,
            }
        );
        assert!(status.is_partial());
        assert_eq!(status.issue_count(), 1);
    }
}
