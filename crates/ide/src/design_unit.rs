//! This-file compilation-unit names.
//!
//! A declaration name, and an instantiation / package token whose candidates
//! all live in this file, are facts of this file (kinds, header text). Jumping
//! to a different file's module is lookup on the closure compilation, not a
//! catalog identity. `hit_global` remains for find-references.

use design_graph::{CursorHit, InstantiationRole, UnitId, UnitKind, hit_global, hit_local};
use nohash_hasher::IntMap;
use smallvec::SmallVec;
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
    this_file_units(db, file_id, offset).map(|(units, range)| {
        RangeInfo::new(range, units.into_iter().map(|unit| nav_from_unit(db, unit)).collect())
    })
}

pub(crate) fn hover(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Markup>> {
    this_file_units(db, file_id, offset)
        .map(|(units, range)| RangeInfo::new(range, hover_targets(db, &units)))
}

fn this_file_units(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    offset: TextSize,
) -> Option<(SmallVec<[UnitId; 2]>, TextRange)> {
    let facts = db.file_facts(file_id);
    if let Some(CursorHit::DeclName { unit, range }) = hit_local(&facts, offset)
        && unit.file == file_id
    {
        return Some((SmallVec::from_elem(unit, 1), range));
    }
    match hit_global(&facts, &db.unit_catalog(), offset) {
        CursorHit::InstantiationType { range, name, role }
            if locator_files_are(db, file_id, |catalog| catalog.files_for_role(&name, role)) =>
        {
            let units = facts.units_for_role(&name, role);
            (!units.is_empty()).then_some((units, range))
        }
        CursorHit::PackageRef { name, range }
            if locator_files_are(db, file_id, |catalog| {
                catalog.files_named_matching(&name, |kind| kind.is_package())
            }) =>
        {
            let units = facts.package_units(&name);
            (!units.is_empty()).then_some((units, range))
        }
        _ => None,
    }
}

fn locator_files_are(
    db: &AnalysisContext<'_>,
    file_id: FileId,
    files: impl FnOnce(&design_graph::UnitCatalog) -> smallvec::SmallVec<[FileId; 2]>,
) -> bool {
    let files = files(&db.unit_catalog());
    !files.is_empty() && files.iter().all(|&file| file == file_id)
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
        CursorHit::InstantiationType { range, name, role } => Some(vec![references_for_units(
            db,
            &units_from_locator(db, &name, role),
            range,
            config,
        )]),
        CursorHit::PackageRef { name, range } => {
            Some(vec![references_for_units(db, &packages_from_locator(db, &name), range, config)])
        }
    }
}

fn hit(db: &AnalysisContext<'_>, file_id: FileId, offset: TextSize) -> CursorHit {
    let facts = db.file_facts(file_id);
    // A declaration name is a fact of this file. ProductStore::transition
    // owns the revision order; this is a cheap local answer, not a race
    // bypass.
    if let Some(hit) = hit_local(&facts, offset) {
        return hit;
    }
    let graph = db.unit_catalog();
    let hit = hit_global(&facts, &graph, offset);
    let hit_kind = match &hit {
        CursorHit::DeclName { .. } => "decl_name",
        CursorHit::InstantiationType { .. } => "instantiation_type",
        CursorHit::PackageRef { .. } => "package_ref",
        CursorHit::Other => "other",
    };
    tracing::debug!(hit_kind, "design_graph.hit");
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
    // A FileFacts node is a source declaration. Generated units have no
    // FileFacts row. Do not fold the workspace graph to learn that — DeclName
    // hover already refused the fold in `hit`.
    let facts = db.file_facts(unit.file);
    let node = facts.unit(unit.clone());
    let text = db.file_text(unit.file);
    let header = node.and_then(|node| node.header_range).and_then(|header| {
        let start = usize::from(header.start());
        let end = usize::from(header.end());
        text.get(start..end)
    });
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
    let graph = db.unit_catalog();
    let def: Vec<NavTarget> = units.iter().cloned().map(|unit| nav_from_unit(db, unit)).collect();
    let mut refs: IntMap<FileId, Vec<(TextRange, ReferenceCategory)>> = IntMap::default();
    for file in reference_files(db, config) {
        let facts = db.file_facts(file);
        for site in facts.instantiations.iter() {
            if units.iter().any(|unit| instantiation_refers_to(graph.as_ref(), site, unit)) {
                refs.entry(file).or_default().push((site.range, ReferenceCategory::empty()));
            }
        }
        for import in facts.imports.iter() {
            if units.iter().any(|unit| package_refers_to(graph.as_ref(), &import.package, unit)) {
                refs.entry(file).or_default().push((import.range, ReferenceCategory::empty()));
            }
        }
        for site in facts.package_refs.iter() {
            if units.iter().any(|unit| package_refers_to(graph.as_ref(), &site.name, unit)) {
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

fn units_from_locator(
    db: &AnalysisContext<'_>,
    name: &str,
    role: InstantiationRole,
) -> SmallVec<[UnitId; 2]> {
    db.unit_catalog()
        .files_for_role(name, role)
        .into_iter()
        .flat_map(|file| db.file_facts(file).units_for_role(name, role))
        .collect()
}

fn packages_from_locator(db: &AnalysisContext<'_>, name: &str) -> SmallVec<[UnitId; 2]> {
    db.unit_catalog()
        .files_named_matching(name, |kind| kind.is_package())
        .into_iter()
        .flat_map(|file| db.file_facts(file).package_units(name))
        .collect()
}

fn instantiation_refers_to(
    catalog: &design_graph::UnitCatalog,
    site: &design_graph::InstantiationSite,
    unit: &UnitId,
) -> bool {
    site.name == unit.name
        && catalog.files_for_role(&site.name, site.role).contains(&unit.file)
        && match site.role {
            InstantiationRole::Hierarchy => unit.kind.is_hierarchy_target(),
            InstantiationRole::Checker => matches!(unit.kind, UnitKind::Checker),
        }
}

fn package_refers_to(catalog: &design_graph::UnitCatalog, name: &str, unit: &UnitId) -> bool {
    unit.kind.is_package()
        && unit.name == name
        && catalog.files_named_matching(name, |kind| kind.is_package()).contains(&unit.file)
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
        CursorHit::InstantiationType { name, role, .. } => {
            let units = units_from_locator(db, &name, role);
            !units.is_empty() && units.iter().all(|unit| is_source_unit(db, unit))
        }
        CursorHit::PackageRef { name, .. } => {
            let units = packages_from_locator(db, &name);
            !units.is_empty() && units.iter().all(|unit| is_source_unit(db, unit))
        }
    }
}

fn is_source_unit(db: &AnalysisContext<'_>, unit: &UnitId) -> bool {
    db.file_facts(unit.file).unit(unit.clone()).is_some()
}

pub(crate) fn rename_guard(
    db: &AnalysisContext<'_>,
    FilePosition { file_id, offset }: FilePosition,
) -> Result<(), crate::rename::RenameError> {
    db.store.record_paid_file(file_id);
    match hit(db, file_id, offset) {
        CursorHit::Other => Ok(()),
        CursorHit::DeclName { unit, .. } => reject_generated(db, &[unit]),
        CursorHit::InstantiationType { name, role, .. } => {
            reject_generated(db, &units_from_locator(db, &name, role))
        }
        CursorHit::PackageRef { name, .. } => {
            reject_generated(db, &packages_from_locator(db, &name))
        }
    }
}

fn reject_generated(
    db: &AnalysisContext<'_>,
    units: &[UnitId],
) -> Result<(), crate::rename::RenameError> {
    if units.iter().any(|unit| db.file_facts(unit.file).unit(unit.clone()).is_none()) {
        return Err(crate::rename::RenameError::MacroDefinitionNotEditable);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{position, setup_marked};

    #[test]
    fn module_decl_hover_uses_file_facts_header() {
        let (host, file_id, _text, markers) = setup_marked(
            "module /*marker:name*/top #(parameter int W = 1);\n  wire unused;\nendmodule\n",
        );
        let hover = host
            .make_analysis()
            .hover(position(file_id, &markers, "name"))
            .unwrap()
            .expect("module name hover");
        let info = hover.info.as_str();
        assert!(info.contains("module top"), "{info}");
        assert!(info.contains("parameter int W = 1"), "{info}");
        assert!(!info.contains("wire unused"), "{info}");
    }
}
