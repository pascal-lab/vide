use hir::{
    base_db::source_db::{SourceDb, SourceRootDb},
    container::InFile,
    file::HirFileId,
    semantics::Semantics,
    source_resolver::PositionResolver,
};
use itertools::Itertools;
use source_model::{
    EntityId, FilePosition as SourceFilePosition, ResolvedSourceTarget, SourceContext,
    SourceEntity, SourcePurpose, SourceRangeResult, SourceTarget as GraphSourceTarget,
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
    syntax_targets::{SyntaxTarget, generated_syntax_target_at_offset, syntax_target_at_offset},
};

enum DefinitionTarget<'tree> {
    Graph(RangeInfo<Vec<NavTarget>>),
    Source(SyntaxTarget<'tree>),
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
        generated_syntax_target_at_offset(db, file_id, root, offset, SourcePurpose::GotoDefinition)
            .or_else(|| syntax_target_at_offset(root, offset, token_precedence))?;
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
            dispatch_graph_definition_target(db, file_id, target)
        }
        GraphSourceTargetResolution::Ambiguous(targets) => targets
            .into_iter()
            .find_map(|target| dispatch_graph_definition_target(db, file_id, target)),
        GraphSourceTargetResolution::Blocked(_) | GraphSourceTargetResolution::None => None,
    }
}

fn dispatch_graph_definition_target(
    db: &RootDb,
    file_id: FileId,
    target: ResolvedSourceTarget,
) -> Option<DefinitionTarget<'static>> {
    match target.target {
        GraphSourceTarget::MacroParamDefinition(_) => {
            dispatch_graph_macro_param_definition_target(db, file_id, target)
                .map(DefinitionTarget::Graph)
        }
        GraphSourceTarget::MacroParamReference(_) => {
            dispatch_graph_macro_param_reference_target(db, file_id, target)
                .map(DefinitionTarget::Graph)
        }
        GraphSourceTarget::MacroDefinition(_) => {
            dispatch_graph_macro_definition_target(db, file_id, target).map(DefinitionTarget::Graph)
        }
        GraphSourceTarget::MacroReference(_) => {
            dispatch_graph_macro_reference_target(db, file_id, target).map(DefinitionTarget::Graph)
        }
        GraphSourceTarget::Include(id) => {
            dispatch_graph_include_target(db, file_id, target, id).map(DefinitionTarget::Graph)
        }
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
    target: SyntaxTarget<'_>,
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
    _file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(reference.model_file_id);
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

fn dispatch_graph_macro_param_definition_target(
    db: &RootDb,
    _file_id: FileId,
    definition: ResolvedSourceTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(definition.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(definition_range) =
        graph.entity_focus_file_range(definition.entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let target = graph_macro_param_nav_target(db, graph, definition.entity)?;
    Some(RangeInfo::new(definition_range.range, vec![target]))
}

fn dispatch_graph_macro_param_reference_target(
    db: &RootDb,
    _file_id: FileId,
    reference: ResolvedSourceTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(reference.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(reference_range) =
        graph.entity_focus_file_range(reference.entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let targets = graph
        .resolved_definitions(source_graph.root_context, reference.entity)
        .iter()
        .filter_map(|(definition, _)| graph_macro_param_nav_target(db, graph, *definition))
        .unique()
        .collect_vec();
    (!targets.is_empty()).then_some(RangeInfo::new(reference_range.range, targets))
}

fn dispatch_graph_macro_definition_target(
    db: &RootDb,
    _file_id: FileId,
    definition: ResolvedSourceTarget,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(definition.model_file_id);
    let source_graph = source_graph.as_ref().as_ref().ok()?;
    let graph = &source_graph.graph;
    let SourceRangeResult::Mapped(definition_range) =
        graph.entity_focus_file_range(definition.entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let target = graph_macro_nav_target(db, graph, definition.entity)?;
    Some(RangeInfo::new(definition_range.range, vec![target]))
}

fn graph_macro_param_nav_target(
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
    let container_name = graph
        .entity_parents(entity)
        .iter()
        .find_map(|parent| graph_macro_entity_name(db, graph, *parent));
    Some(NavTarget {
        file_id: file_range.file_id,
        full_range: file_range.range,
        focus_range: Some(file_range.range),
        name: Some(name.into()),
        kind: None,
        container_name,
        description: Some("macro parameter".to_owned()),
    })
}

fn graph_macro_entity_name(
    db: &RootDb,
    graph: &source_model::SourceGraph,
    entity: EntityId,
) -> Option<smol_str::SmolStr> {
    let SourceEntity::MacroDefinition(_) = graph.entity(entity) else {
        return None;
    };
    let SourceRangeResult::Mapped(file_range) =
        graph.entity_focus_file_range(entity, SourcePurpose::GotoDefinition)
    else {
        return None;
    };
    let text = db.file_text(file_range.file_id);
    Some(text[file_range.range].to_owned().into())
}

fn dispatch_graph_include_target(
    db: &RootDb,
    _file_id: FileId,
    include: ResolvedSourceTarget,
    include_id: source_model::IncludeDirectiveId,
) -> Option<RangeInfo<Vec<NavTarget>>> {
    let source_graph = db.source_graph_preproc_model(include.model_file_id);
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
