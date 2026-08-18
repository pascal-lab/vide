//! Compilation-unit name navigation through `hit_at`.
//!
//! This is the only CU-name answer. Empty graph candidates are `Other` — a
//! different question (nested module, class `::`, UDP), not a second path.

use design_graph::{CursorHit, UnitId, UnitKind, UnitOrigin, hit_at};
use nohash_hasher::IntMap;
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    analysis::AnalysisContext,
    markup::Markup,
    navigation_target::NavTarget,
    references::{ReferenceCategory, References, ReferencesConfig, ReferencesStatus},
};

pub(crate) fn goto_definition(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    match hit(db, file_id, offset) {
        CursorHit::Other => None,
        CursorHit::DeclName { unit, range } => {
            Some(RangeInfo::new(range, vec![nav_from_unit(db, unit)]))
        }
        CursorHit::InstantiationType { range, targets }
        | CursorHit::PackageRef { range, targets, .. } => {
            let navs: Vec<_> = targets.into_iter().map(|unit| nav_from_unit(db, unit)).collect();
            Some(RangeInfo::new(range, navs))
        }
    }
}

pub(crate) fn hover(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Markup>> {
    match hit(db, file_id, offset) {
        CursorHit::Other => None,
        CursorHit::DeclName { unit, range } => Some(RangeInfo::new(range, hover_markup(db, &unit))),
        CursorHit::InstantiationType { range, targets }
        | CursorHit::PackageRef { range, targets, .. } => {
            Some(RangeInfo::new(range, hover_targets(db, &targets)))
        }
    }
}

pub(crate) fn references(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
    config: &ReferencesConfig,
) -> Option<Vec<References>> {
    match hit(db, file_id, offset) {
        CursorHit::Other => None,
        CursorHit::DeclName { unit, range } => {
            Some(vec![references_for_units(db, &[unit], range, config)])
        }
        CursorHit::InstantiationType { range, targets }
        | CursorHit::PackageRef { range, targets, .. } => {
            Some(vec![references_for_units(db, &targets, range, config)])
        }
    }
}

fn hit(db: &AnalysisContext<'_>, file_id: FileId, offset: TextSize) -> CursorHit {
    let facts = db.file_facts(file_id);
    let graph = db.design_graph();
    let hit = hit_at(&facts, &graph, file_id, offset);
    let (hit_kind, target_count) = match &hit {
        CursorHit::DeclName { .. } => ("decl_name", 1usize),
        CursorHit::InstantiationType { targets, .. } => ("instantiation_type", targets.len()),
        CursorHit::PackageRef { targets, .. } => ("package_ref", targets.len()),
        CursorHit::Other => ("other", 0usize),
    };
    tracing::debug!(hit_kind, target_count, "design_graph.hit");
    hit
}

pub(crate) fn nav_from_unit(db: &AnalysisContext<'_>, unit: UnitId) -> NavTarget {
    let facts = db.file_facts(unit.file);
    let node = facts.unit(unit.clone());
    let name_range = node.and_then(|node| node.name_range);
    let range = name_range.unwrap_or_else(|| TextRange::empty(TextSize::new(0)));
    NavTarget {
        file_id: unit.file,
        full_range: range,
        focus_range: name_range,
        name: Some(unit.name.clone()),
        kind: def_kind(unit.kind),
        container_name: None,
        description: None,
    }
}

fn hover_targets(db: &AnalysisContext<'_>, targets: &[UnitId]) -> Markup {
    let mut markup = Markup::new();
    for (index, unit) in targets.iter().enumerate() {
        if index > 0 {
            markup.horizontal_line();
        }
        markup.merge(hover_markup(db, unit));
    }
    markup
}

fn hover_markup(db: &AnalysisContext<'_>, unit: &UnitId) -> Markup {
    let facts = db.file_facts(unit.file);
    let node = facts.unit(unit.clone());
    let origin = db.design_graph().origin(unit).unwrap_or(UnitOrigin::Source);
    let text = db.file_text(unit.file);
    let header = match origin {
        UnitOrigin::Generated => None,
        UnitOrigin::Source => node.and_then(|node| node.header_range).and_then(|header| {
            let start = usize::from(header.start());
            let end = usize::from(header.end());
            text.get(start..end)
        }),
    };
    let header =
        header.map(str::trim_end).filter(|header| !header.is_empty()).unwrap_or(unit.name.as_str());
    let mut markup = Markup::new();
    markup.push_with_code_fence(header);
    let range =
        node.and_then(|node| node.name_range).unwrap_or_else(|| TextRange::empty(TextSize::new(0)));
    if let Some(link) = crate::render::source_location_link(db, unit.file, range.start(), unit.file)
    {
        markup.metadata_line(&format!("from {link}"));
    }
    markup
}

fn references_for_units(
    db: &AnalysisContext<'_>,
    units: &[UnitId],
    _caret_range: TextRange,
    config: &ReferencesConfig,
) -> References {
    let graph = db.design_graph();
    let def: Vec<NavTarget> = units.iter().cloned().map(|unit| nav_from_unit(db, unit)).collect();
    let mut refs: IntMap<FileId, Vec<(TextRange, ReferenceCategory)>> = IntMap::default();
    for file in reference_files(db, config) {
        let facts = db.file_facts(file);
        for site in facts.instantiations.iter() {
            let targets = graph.candidates(&site.name, site.role);
            if units.iter().any(|unit| targets.iter().any(|target| target == unit)) {
                refs.entry(file).or_default().push((site.range, ReferenceCategory::empty()));
            }
        }
        for import in facts.imports.iter() {
            let targets = graph.packages_named(&import.package).into_vec();
            if units.iter().any(|unit| targets.iter().any(|target| target == unit)) {
                refs.entry(file).or_default().push((import.range, ReferenceCategory::empty()));
            }
        }
        for site in facts.package_refs.iter() {
            let targets = graph.packages_named(&site.name).into_vec();
            if units.iter().any(|unit| targets.iter().any(|target| target == unit)) {
                refs.entry(file).or_default().push((site.range, ReferenceCategory::empty()));
            }
        }
    }
    for unit in units {
        if let Some(range) =
            db.file_facts(unit.file).unit(unit.clone()).and_then(|node| node.name_range)
            && let Some(hits) = refs.get_mut(&unit.file)
        {
            hits.retain(|(hit, _)| *hit != range);
            if hits.is_empty() {
                refs.remove(&unit.file);
            }
        }
    }
    refs.retain(|_, hits| !hits.is_empty());
    References { def: Some(def), refs, status: ReferencesStatus::Complete }
}

fn reference_files(db: &AnalysisContext<'_>, config: &ReferencesConfig) -> Vec<FileId> {
    if let Some(scope) = &config.search_scope {
        return scope.files().collect();
    }
    db.files()
        .iter()
        .copied()
        .filter(|&file| db.file_kind(file).is_semantic_compilation_unit())
        .collect()
}

fn def_kind(kind: UnitKind) -> Option<crate::DefKind> {
    match kind {
        UnitKind::Module => Some(crate::DefKind::Module),
        UnitKind::Interface => Some(crate::DefKind::Interface),
        UnitKind::Package => Some(crate::DefKind::Package),
        UnitKind::Program => Some(crate::DefKind::Program),
        UnitKind::Checker => Some(crate::DefKind::Checker),
        UnitKind::Covergroup => Some(crate::DefKind::Covergroup),
    }
}

pub(crate) fn source_visible_hit(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> bool {
    match hit(db, file_id, offset) {
        CursorHit::Other => false,
        CursorHit::DeclName { unit, .. } => is_source_unit(db, &unit),
        CursorHit::InstantiationType { targets, .. } | CursorHit::PackageRef { targets, .. } => {
            !targets.is_empty() && targets.iter().all(|unit| is_source_unit(db, unit))
        }
    }
}

fn is_source_unit(db: &AnalysisContext<'_>, unit: &UnitId) -> bool {
    db.design_graph().origin(unit) != Some(UnitOrigin::Generated)
        && db.file_facts(unit.file).unit(unit.clone()).is_some()
}

pub(crate) fn rename_guard(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Result<(), crate::rename::RenameError> {
    crate::generated_units::record_from_paid_artifact(db, file_id);
    match hit(db, file_id, offset) {
        CursorHit::Other => Ok(()),
        CursorHit::DeclName { unit, .. } => reject_generated(db, &[unit]),
        CursorHit::InstantiationType { targets, .. } | CursorHit::PackageRef { targets, .. } => {
            reject_generated(db, &targets)
        }
    }
}

fn reject_generated(
    db: &AnalysisContext<'_>,
    units: &[UnitId],
) -> Result<(), crate::rename::RenameError> {
    if units.iter().any(|unit| {
        db.design_graph().origin(unit) == Some(UnitOrigin::Generated)
            || db.file_facts(unit.file).unit(unit.clone()).is_none()
    }) {
        return Err(crate::rename::RenameError::MacroDefinitionNotEditable);
    }
    Ok(())
}
