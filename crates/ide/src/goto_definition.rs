use hir::{
    base_db::source_db::{SourceDb, SourceRootDb},
    container::InFile,
    file::HirFileId,
    preproc::{
        IncludeDirective, IncludeTarget, MacroDefinition, MacroParamDefinition,
        MacroParamReferenceDefinitions, MacroReferenceDefinitions, include_directives_at,
        macro_definition_at, macro_param_definition_at, macro_param_reference_definitions_at,
        macro_reference_definitions_at,
    },
    semantics::Semantics,
    source_resolver::PositionResolver,
};
use itertools::Itertools;
use source_model::{
    EntityId, FilePosition as SourceFilePosition, ResolvedSourceTarget, SourceContext,
    SourcePurpose, SourceRangeResult, SourceTarget as GraphSourceTarget,
    SourceTargetResolution as GraphSourceTargetResolution,
};
use syntax::{
    SyntaxNode, SyntaxTokenWithParent, TokenKind,
    token::{TokenKindExt, pair_token},
};
use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::{
    FilePosition, RangeInfo,
    db::root_db::RootDb,
    definitions::DefinitionClass,
    navigation_target::{NavTarget, ToNav},
    source_targets::{SourceTarget, source_target_at_offset},
};

enum DefinitionTarget<'tree> {
    Preproc(Box<PreprocDefinitionTarget>),
    Include(Vec<IncludeDirective>),
    Graph(RangeInfo<Vec<NavTarget>>),
    Source(SourceTarget<'tree>),
}

enum PreprocDefinitionTarget {
    ParamDefinition(MacroParamDefinition),
    ParamReference(MacroParamReferenceDefinitions),
    Definition(MacroDefinition),
    Reference(MacroReferenceDefinitions),
}

pub(crate) fn goto_definition(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let target = dispatch_definition_target(db, file_id, offset, parsed_file.root())?;
    render_definition_target(db, file_id, &sema, target)
}

fn dispatch_definition_target<'tree>(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    root: Option<SyntaxNode<'tree>>,
) -> Option<DefinitionTarget<'tree>> {
    if let Some(target) = dispatch_source_graph_definition_target(db, file_id, offset) {
        return Some(target);
    }
    let root = root?;
    let target =
        source_target_at_offset(db, file_id, root, offset, token_precedence)?.resolved()?;
    Some(DefinitionTarget::Source(target))
}

fn dispatch_source_graph_definition_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<DefinitionTarget<'static>> {
    let resolution = PositionResolver::new(db).resolve_position(
        SourceFilePosition { file_id, offset },
        SourcePurpose::GotoDefinition,
        None,
    );

    match resolution {
        GraphSourceTargetResolution::Resolved(target) => {
            dispatch_graph_definition_target(db, file_id, offset, target)
        }
        GraphSourceTargetResolution::Ambiguous(targets) => targets
            .into_iter()
            .find_map(|target| dispatch_graph_definition_target(db, file_id, offset, target)),
        GraphSourceTargetResolution::Blocked(_) | GraphSourceTargetResolution::None => None,
    }
}

fn dispatch_graph_definition_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
    target: ResolvedSourceTarget,
) -> Option<DefinitionTarget<'static>> {
    match target.target {
        GraphSourceTarget::MacroParamDefinition(_) => {
            dispatch_macro_param_definition_target(db, file_id, offset)
                .map(|target| DefinitionTarget::Preproc(Box::new(target)))
        }
        GraphSourceTarget::MacroParamReference(_) => {
            dispatch_macro_param_reference_target(db, file_id, offset)
                .map(|target| DefinitionTarget::Preproc(Box::new(target)))
        }
        GraphSourceTarget::MacroDefinition(_) => {
            dispatch_macro_definition_target(db, file_id, offset)
                .map(|target| DefinitionTarget::Preproc(Box::new(target)))
        }
        GraphSourceTarget::MacroReference(_) => {
            dispatch_graph_macro_reference_target(db, file_id, target)
                .map(DefinitionTarget::Graph)
                .or_else(|| {
                    dispatch_macro_reference_target(db, file_id, offset)
                        .map(|target| DefinitionTarget::Preproc(Box::new(target)))
                })
        }
        GraphSourceTarget::Include(id) => dispatch_graph_include_target(db, file_id, target, id)
            .map(DefinitionTarget::Graph)
            .or_else(|| {
                dispatch_include_definition_target(db, file_id, offset)
                    .map(DefinitionTarget::Include)
            }),
        GraphSourceTarget::MacroCall(_)
        | GraphSourceTarget::ExpansionToken(_)
        | GraphSourceTarget::HirSymbol(_)
        | GraphSourceTarget::HirReference(_)
        | GraphSourceTarget::SyntaxToken(_) => None,
    }
}

fn render_definition_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: DefinitionTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    match target {
        DefinitionTarget::Preproc(target) => render_preproc_definition_target(*target),
        DefinitionTarget::Include(includes) => render_include_definition_target(db, includes),
        DefinitionTarget::Graph(target) => Some(target),
        DefinitionTarget::Source(target) => {
            render_source_definition_target(db, file_id, sema, target)
        }
    }
}

fn render_source_definition_target(
    db: &RootDb,
    file_id: FileId,
    sema: &Semantics<RootDb>,
    target: SourceTarget<'_>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let hir_file_id = file_id.into();
    let (range, tokens) = target.into_parts();
    let navs = tokens
        .into_iter()
        .filter_map(|token| nav_targets_for_token(db, sema, hir_file_id, token))
        .flatten()
        .unique()
        .collect_vec();
    if navs.is_empty() {
        return None;
    }

    Some(RangeInfo::new(range, navs))
}

fn nav_targets_for_token(
    db: &RootDb,
    sema: &Semantics<RootDb>,
    hir_file_id: HirFileId,
    token: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    handle_ctrl_flow_kw(sema, hir_file_id, token).or_else(|| {
        DefinitionClass::resolve(sema, hir_file_id, token)?
            .origins()
            .into_iter()
            .unique()
            .filter_map(|def| def.to_nav(db))
            .collect_vec()
            .into()
    })
}

fn dispatch_graph_macro_reference_target(
    db: &RootDb,
    file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(reference_range) =
        graph.entity_full_file_range(reference.entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let targets = graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .filter_map(|(definition, _)| graph_macro_nav_target(db, graph, *definition))
        .unique()
        .collect_vec();
    (!targets.is_empty()).then_some(RangeInfo::new(reference_range.range, targets))
}

fn graph_macro_nav_target(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: EntityId,
) -> Option<NavTarget> {
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let text = db.file_text(file_range.file_id);
    let name = text[file_range.range].to_owned();
    Some(NavTarget {
        file_id: file_range.file_id,
        full_range: file_range.range,
        focus_range: Some(file_range.range),
        name: Some(name.into()),
        kind: None,
        container_name: None,
        description: Some("macro definition".to_owned()),
    })
}

fn dispatch_macro_param_definition_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocDefinitionTarget> {
    if let Ok(Some(definition)) = macro_param_definition_at(db, file_id, offset) {
        return Some(PreprocDefinitionTarget::ParamDefinition(definition));
    }
    None
}

fn dispatch_macro_param_reference_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocDefinitionTarget> {
    if let Ok(Some(resolution)) = macro_param_reference_definitions_at(db, file_id, offset) {
        if resolution.definitions.is_empty() {
            return None;
        }
        return Some(PreprocDefinitionTarget::ParamReference(resolution));
    }
    None
}

fn dispatch_macro_definition_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocDefinitionTarget> {
    if let Ok(Some(definition)) = macro_definition_at(db, file_id, offset) {
        return Some(PreprocDefinitionTarget::Definition(definition));
    }
    None
}

fn dispatch_macro_reference_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<PreprocDefinitionTarget> {
    if let Ok(Some(resolution)) = macro_reference_definitions_at(db, file_id, offset) {
        if resolution.definitions.is_empty() {
            return None;
        }
        return Some(PreprocDefinitionTarget::Reference(resolution));
    }
    None
}

fn render_preproc_definition_target(
    target: PreprocDefinitionTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    match target {
        PreprocDefinitionTarget::ParamDefinition(definition) => {
            Some(RangeInfo::new(definition.range, vec![macro_param_nav_target(definition)]))
        }
        PreprocDefinitionTarget::ParamReference(resolution) => {
            let reference_range = resolution.range;
            let targets =
                resolution.definitions.into_iter().map(macro_param_nav_target).collect_vec();
            (!targets.is_empty()).then_some(RangeInfo::new(reference_range, targets))
        }
        PreprocDefinitionTarget::Definition(definition) => {
            Some(RangeInfo::new(definition.name_range, vec![macro_nav_target(definition)]))
        }
        PreprocDefinitionTarget::Reference(resolution) => {
            let reference_range = resolution.range;
            let targets = resolution.definitions.into_iter().map(macro_nav_target).collect_vec();
            (!targets.is_empty()).then_some(RangeInfo::new(reference_range, targets))
        }
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

fn dispatch_graph_include_target(
    db: &RootDb,
    file_id: FileId,
    include: ResolvedSourceTarget,
    include_id: source_model::IncludeDirectiveId,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(include_range) =
        graph.entity_focus_file_range(include.entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let included_context = graph.included_context(source_graph.root_context, include_id)?;
    let SourceContext::IncludeContext { included_file, .. } = *graph.context(included_context)
    else {
        return None;
    };
    let target_range = TextRange::empty(TextSize::new(0));
    let include_text = db.file_text(include_range.file_id);
    let name = include_text[include_range.range].trim_matches('"').to_owned();
    let target = NavTarget {
        file_id: included_file,
        full_range: target_range,
        focus_range: Some(target_range),
        name: Some(name.into()),
        kind: None,
        container_name: None,
        description: db.file_path(included_file).map(|path| path.to_string()),
    };
    Some(RangeInfo::new(include_range.range, vec![target]))
}

fn dispatch_include_definition_target(
    db: &RootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<Vec<IncludeDirective>> {
    let includes = include_directives_at(db, file_id, offset).ok()?;
    (!includes.is_empty()).then_some(includes)
}

fn render_include_definition_target(
    db: &RootDb,
    includes: Vec<IncludeDirective>,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let range = includes.first()?.range;
    let targets = includes
        .into_iter()
        .filter_map(|include| {
            let IncludeTarget::Literal { path, resolved_file: Some(target_file_id) } =
                include.target
            else {
                return None;
            };
            let target_range = TextRange::empty(TextSize::new(0));
            Some(NavTarget {
                file_id: target_file_id,
                full_range: target_range,
                focus_range: Some(target_range),
                name: Some(path),
                kind: None,
                container_name: None,
                description: db.file_path(target_file_id).map(|path| path.to_string()),
            })
        })
        .unique()
        .collect_vec();
    if targets.is_empty() {
        return None;
    }
    Some(RangeInfo::new(range, targets))
}

fn handle_ctrl_flow_kw(
    sema: &Semantics<RootDb>,
    file_id: HirFileId,
    tp @ SyntaxTokenWithParent { .. }: SyntaxTokenWithParent,
) -> Option<Vec<NavTarget>> {
    let kind = tp.kind();

    match kind {
        _ if let Some(pair) = pair_token(tp) => {
            let tok = InFile::new(file_id, pair.either(|pair| pair, |_| tp));
            Some(vec![tok.to_nav(sema.db)?])
        }
        _ => None,
    }
}

pub(crate) fn token_precedence(kind: TokenKind) -> usize {
    match kind {
        _ if kind.name_like() => 4,
        _ if kind.is_pair_token() => 4,
        _ => 1,
    }
}
