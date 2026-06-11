use source_model::{
    FilePosition, ResolvedSourceTarget, SourceBlock, SourceBlockReason, SourceContextId,
    SourceEntity, SourcePurpose, SourceTarget, SourceTargetResolution,
};
use vfs::FileId;

use crate::base_db::source_db::{SourceFileKind, SourcePreprocQueryError, SourceRootDb};

#[derive(Debug, Clone, Copy)]
pub struct PositionResolver<'db> {
    db: &'db dyn SourceRootDb,
}

impl<'db> PositionResolver<'db> {
    pub fn new(db: &'db dyn SourceRootDb) -> Self {
        Self { db }
    }

    pub fn resolve_position(
        &self,
        position: FilePosition,
        purpose: SourcePurpose,
        context: Option<SourceContextId>,
    ) -> SourceTargetResolution {
        resolve_position(self.db, position, purpose, context)
    }
}

pub fn resolve_position(
    db: &dyn SourceRootDb,
    position: FilePosition,
    purpose: SourcePurpose,
    context: Option<SourceContextId>,
) -> SourceTargetResolution {
    let model_file_ids = source_graph_model_file_ids_for_file(db, position.file_id);
    let mut first_error = None;
    let mut targets = Vec::new();

    for model_file_id in model_file_ids {
        let source_graph = db.source_graph_preproc_model(model_file_id);
        let source_graph = match source_graph.as_ref() {
            Ok(source_graph) => source_graph,
            Err(SourcePreprocQueryError::UnsupportedFileKind(_)) => continue,
            Err(err) => {
                first_error.get_or_insert_with(|| err.clone());
                continue;
            }
        };

        let graph = &source_graph.graph;
        targets.extend(
            graph
                .entities_at_file_position(position, context.or(Some(source_graph.root_context)))
                .into_iter()
                .filter_map(|hit| {
                    let target = source_target_for_entity(graph.entity(hit.entity))?;
                    Some(ResolvedSourceTarget { model_file_id, entity: hit.entity, target })
                }),
        );
    }

    if targets.is_empty() && first_error.is_some() {
        return SourceTargetResolution::Blocked(SourceBlock {
            reason: SourceBlockReason::Unavailable(source_model::SourceUnavailable::Unsupported),
            preferred_span: None,
        });
    }
    targets.sort_by_key(|target| target_rank(target.target, purpose));
    targets.dedup();

    let Some(best) = targets.first().copied() else {
        return SourceTargetResolution::None;
    };
    let best_rank = target_rank(best.target, purpose);
    let best_targets = targets
        .into_iter()
        .take_while(|target| target_rank(target.target, purpose) == best_rank)
        .collect::<Vec<_>>();

    match best_targets.as_slice() {
        [target] => SourceTargetResolution::Resolved(*target),
        [] => SourceTargetResolution::None,
        _ => SourceTargetResolution::Ambiguous(best_targets),
    }
}

pub fn source_graph_model_file_ids_for_file(db: &dyn SourceRootDb, file_id: FileId) -> Vec<FileId> {
    let relevant = db.source_preproc_contexts_for_file(file_id);
    let mut file_ids = Vec::new();
    let profile_id = db.file_compilation_profile(file_id);
    let plan = db.compilation_plan_for_profile(profile_id);
    let is_include_only = plan.include_only.contains(&file_id);
    let include_self = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog if !is_include_only => true,
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            relevant.model_file_ids.is_empty()
        }
        _ => false,
    };
    if include_self {
        file_ids.push(file_id);
    }
    for model_file_id in relevant.model_file_ids.iter().copied() {
        if !file_ids.contains(&model_file_id) {
            file_ids.push(model_file_id);
        }
    }
    file_ids
}

fn source_target_for_entity(entity: SourceEntity) -> Option<SourceTarget> {
    Some(match entity {
        SourceEntity::MacroDefinition(id) => SourceTarget::MacroDefinition(id),
        SourceEntity::MacroReference(id) => SourceTarget::MacroReference(id),
        SourceEntity::MacroCall(id) => SourceTarget::MacroCall(id),
        SourceEntity::MacroParamDefinition(id) => SourceTarget::MacroParamDefinition(id),
        SourceEntity::MacroParamReference(id) => SourceTarget::MacroParamReference(id),
        SourceEntity::IncludeDirective(id) => SourceTarget::Include(id),
        SourceEntity::ExpansionToken(id) => SourceTarget::ExpansionToken(id),
        SourceEntity::HirSymbol(id) => SourceTarget::HirSymbol(id),
        SourceEntity::HirReference(id) => SourceTarget::HirReference(id),
        SourceEntity::SyntaxToken(id) => SourceTarget::SyntaxToken(id),
        SourceEntity::InactiveRegion(_) => return None,
    })
}

fn target_rank(target: SourceTarget, purpose: SourcePurpose) -> u8 {
    match (purpose, target) {
        (_, SourceTarget::MacroParamReference(_)) => 0,
        (_, SourceTarget::MacroParamDefinition(_)) => 0,
        (_, SourceTarget::MacroReference(_)) => 1,
        (_, SourceTarget::MacroDefinition(_)) => 1,
        (_, SourceTarget::Include(_)) => 1,
        (_, SourceTarget::HirReference(_)) => 1,
        (_, SourceTarget::HirSymbol(_)) => 1,
        (_, SourceTarget::ExpansionToken(_)) => 2,
        (_, SourceTarget::SyntaxToken(_)) => 2,
        (_, SourceTarget::MacroCall(_)) => 2,
    }
}
