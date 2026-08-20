use hir_def::{owner::OwnerId, symbol::DefKind};

use crate::analysis::AnalysisContext;

pub(super) fn value_candidates_in_module(
    db: &AnalysisContext<'_>,
    module_id: OwnerId,
) -> Vec<String> {
    names_in_module(db, module_id, |kind| {
        matches!(
            kind,
            DefKind::Variable
                | DefKind::Net
                | DefKind::Genvar
                | DefKind::Specparam
                | DefKind::Port
                | DefKind::NonAnsiPort
        )
    })
}

pub(super) fn const_candidates_in_module(
    db: &AnalysisContext<'_>,
    module_id: OwnerId,
) -> Vec<String> {
    names_in_module(db, module_id, |kind| kind == DefKind::Param)
}

fn names_in_module(
    db: &AnalysisContext<'_>,
    module_id: OwnerId,
    include: impl Fn(DefKind) -> bool,
) -> Vec<String> {
    let scope = db.scope(module_id);
    let mut candidates: Vec<_> = scope
        .iter_listing()
        .filter_map(|(name, defs)| {
            defs.into_iter().any(|def| include(def.kind(db.db))).then(|| name.to_string())
        })
        .collect();
    candidates.sort();
    candidates.dedup();
    candidates
}
