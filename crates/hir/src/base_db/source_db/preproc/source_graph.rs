use preproc::source::{
    PreprocSourceId, SourceIncludeDirectiveId, SourceIncludeStatus, SourceMacroResolution,
    SourceRange,
};
use rustc_hash::FxHashMap;
use source_model::{
    EntityId, InactiveRegionId, IncludeDirectiveId, MacroCallId, MacroDefinitionId,
    MacroExpansionId, MacroReferenceId, ResolutionReason, SourceContext, SourceContextId,
    SourceDomain, SourceDomainId, SourceEntity, SourceGraph, SourceGraphBuilder, SourceRelation,
    SourceSelectionId, SourceUnavailable, SpanId, VirtualOrigin,
};
use vfs::FileId;

use super::{
    MappedSourcePreprocModel, PreprocSourceMapping, PreprocVirtualOrigin, SourcePreprocUnavailable,
};
use crate::base_db::project::CompilationProfileId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGraphPreprocModel {
    pub graph: SourceGraph,
    pub root_context: SourceContextId,
    pub source_domains: FxHashMap<PreprocSourceId, SourceDomainId>,
    pub include_contexts: FxHashMap<SourceIncludeDirectiveId, SourceContextId>,
}

pub(super) fn source_graph_preproc_model_from_mapped(
    mapped: &MappedSourcePreprocModel,
    root_file: FileId,
    profile_id: Option<CompilationProfileId>,
) -> SourceGraphPreprocModel {
    let mut builder = SourceGraphBuilder::new();
    let root_context = builder.add_context(SourceContext::CompilationRoot {
        profile_id: profile_id.map(|id| id.0),
        root_file,
    });

    let mut source_domains = FxHashMap::default();
    for (source, mapping) in mapped.source_map.source_mappings() {
        let domain = builder.intern_domain(source_domain_from_preproc_mapping(mapping));
        source_domains.insert(source, domain);
    }

    let mut include_contexts = FxHashMap::default();
    for include in mapped.model.include_graph().directives() {
        if let SourceIncludeStatus::Resolved { source } = include.status {
            let context = builder.add_context(SourceContext::IncludeContext {
                parent: root_context,
                include_directive: IncludeDirectiveId::new(include.id.raw() as u32),
                included_file: mapped.source_map.file_id(source).unwrap_or(root_file),
            });
            include_contexts.insert(include.id, context);
        }
    }

    add_preproc_entities(&mut builder, mapped, root_context, &source_domains, &include_contexts);

    SourceGraphPreprocModel {
        graph: builder.build(),
        root_context,
        source_domains,
        include_contexts,
    }
}

fn add_preproc_entities(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    root_context: SourceContextId,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    include_contexts: &FxHashMap<SourceIncludeDirectiveId, SourceContextId>,
) {
    let mut macro_def_entities = FxHashMap::default();
    let mut macro_ref_entities = FxHashMap::default();
    let mut macro_call_entities = FxHashMap::default();
    for definition in mapped.model.macro_definitions().iter() {
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(
            definition.id.raw() as u32,
        )));
        let selection = intern_selection_for_ranges(
            builder,
            mapped,
            source_domains,
            definition.directive_range,
            Some(definition.name_range),
        );
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
        macro_def_entities.insert(definition.id, entity);
    }

    for reference in mapped.model.macro_references().iter() {
        let entity = builder.add_entity(SourceEntity::MacroReference(MacroReferenceId::new(
            reference.id.raw() as u32,
        )));
        let selection = intern_selection_for_ranges(
            builder,
            mapped,
            source_domains,
            reference.directive_range,
            Some(reference.name_range),
        );
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
        add_macro_resolution_relation(
            builder,
            root_context,
            entity,
            &reference.resolution,
            &macro_def_entities,
        );
        macro_ref_entities.insert(reference.id, entity);
    }

    for call in mapped.model.macro_calls().iter() {
        let entity =
            builder.add_entity(SourceEntity::MacroCall(MacroCallId::new(call.id.raw() as u32)));
        let focus = mapped
            .model
            .macro_references()
            .get(call.reference)
            .map(|reference| reference.name_range);
        let selection =
            intern_selection_for_ranges(builder, mapped, source_domains, call.call_range, focus);
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
        if let Some(reference_entity) = macro_ref_entities.get(&call.reference).copied() {
            builder
                .add_relation(SourceRelation::Contains { parent: entity, child: reference_entity });
        }
        add_macro_resolution_relation(
            builder,
            root_context,
            entity,
            &call.callee,
            &macro_def_entities,
        );
        macro_call_entities.insert(call.id, entity);
    }

    for include in mapped.model.include_graph().directives() {
        let entity = builder.add_entity(SourceEntity::IncludeDirective(IncludeDirectiveId::new(
            include.id.raw() as u32,
        )));
        let selection = intern_selection_for_ranges(
            builder,
            mapped,
            source_domains,
            include.directive_range,
            include.target_range,
        );
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
        if let Some(included_context) = include_contexts.get(&include.id).copied() {
            builder.add_relation(SourceRelation::Includes {
                context: root_context,
                directive: IncludeDirectiveId::new(include.id.raw() as u32),
                included_context,
            });
        }
    }

    for (index, inactive_range) in mapped.model.inactive_ranges().iter().enumerate() {
        let entity =
            builder.add_entity(SourceEntity::InactiveRegion(InactiveRegionId::new(index as u32)));
        let selection =
            intern_selection_for_ranges(builder, mapped, source_domains, *inactive_range, None);
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
    }

    for expansion in mapped.model.macro_expansions().iter() {
        if macro_call_entities.contains_key(&expansion.call) {
            builder.add_relation(SourceRelation::Expands {
                context: root_context,
                call: MacroCallId::new(expansion.call.raw() as u32),
                expansion: MacroExpansionId::new(expansion.id.raw() as u32),
            });
        }
    }
}

fn intern_selection_for_ranges(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    full: SourceRange,
    focus: Option<SourceRange>,
) -> SourceSelectionId {
    let full = intern_span_for_source_range(builder, mapped, source_domains, full);
    let focus =
        focus.map(|range| intern_span_for_source_range(builder, mapped, source_domains, range));
    builder.intern_selection(full, focus)
}

fn intern_span_for_source_range(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    source_range: SourceRange,
) -> SpanId {
    let domain = source_domains.get(&source_range.source).copied().unwrap_or_else(|| {
        builder.intern_domain(SourceDomain::Unmapped {
            reason: SourceUnavailable::MissingSource { source: source_range.source.raw() },
        })
    });
    let range = mapped.source_map.map_range(source_range).unwrap_or(source_range.range);
    builder.intern_span(domain, range)
}

fn add_macro_resolution_relation(
    builder: &mut SourceGraphBuilder,
    context: SourceContextId,
    reference: EntityId,
    resolution: &SourceMacroResolution,
    macro_def_entities: &FxHashMap<preproc::source::SourceMacroDefinitionId, EntityId>,
) {
    let SourceMacroResolution::Resolved { definition, reason, .. } = resolution else {
        return;
    };
    let Some(definition_entity) = macro_def_entities.get(definition).copied() else {
        return;
    };
    builder.add_relation(SourceRelation::ResolvesTo {
        context,
        reference,
        definition: definition_entity,
        reason: match reason {
            preproc::source::SourceMacroResolutionReason::VisibleDefinition => {
                ResolutionReason::VisibleDefinition
            }
            preproc::source::SourceMacroResolutionReason::IncludeGuardIfNDef => {
                ResolutionReason::IncludeGuardIfNDef
            }
        },
    });
}

fn source_domain_from_preproc_mapping(mapping: &PreprocSourceMapping) -> SourceDomain {
    match mapping {
        PreprocSourceMapping::RealFile(file_id) => SourceDomain::RealFile { file_id: *file_id },
        PreprocSourceMapping::VirtualFile { file_id, path, origin } => SourceDomain::VirtualFile {
            file_id: *file_id,
            path: path.clone(),
            origin: virtual_origin_from_preproc(origin),
        },
        PreprocSourceMapping::VirtualDisplay { path, origin } => SourceDomain::VirtualDisplay {
            path: path.clone(),
            origin: virtual_origin_from_preproc(origin),
        },
        PreprocSourceMapping::Unmapped(reason) => {
            SourceDomain::Unmapped { reason: source_unavailable_from_preproc(reason) }
        }
    }
}

fn virtual_origin_from_preproc(origin: &PreprocVirtualOrigin) -> VirtualOrigin {
    match origin {
        PreprocVirtualOrigin::Predefines { profile } => {
            VirtualOrigin::Predefines { profile: profile.map(|id| id.0) }
        }
        PreprocVirtualOrigin::Builtin { name } => VirtualOrigin::Builtin { name: name.clone() },
        PreprocVirtualOrigin::ExternalIncludeBuffer { source } => {
            VirtualOrigin::ExternalIncludeBuffer { source: source.raw() }
        }
        PreprocVirtualOrigin::Expansion { expansion } => VirtualOrigin::Expansion {
            expansion: source_model::MacroExpansionId::new(expansion.raw() as u32),
        },
        PreprocVirtualOrigin::Speculative { universe } => {
            VirtualOrigin::Speculative { universe: universe.0 }
        }
    }
}

fn source_unavailable_from_preproc(reason: &SourcePreprocUnavailable) -> SourceUnavailable {
    match reason {
        SourcePreprocUnavailable::MacroCallAuthorityUnavailable => {
            SourceUnavailable::MacroCallAuthorityUnavailable
        }
        SourcePreprocUnavailable::ExpansionAuthorityUnavailable => {
            SourceUnavailable::ExpansionAuthorityUnavailable
        }
        SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable => {
            SourceUnavailable::TokenProvenanceAuthorityUnavailable
        }
        SourcePreprocUnavailable::DetachedSource { source }
        | SourcePreprocUnavailable::MissingPredefineSourceText { source }
        | SourcePreprocUnavailable::UnverifiedPredefineSource { source } => {
            SourceUnavailable::UnmappedSource { source: source.raw() }
        }
        _ => SourceUnavailable::Unsupported,
    }
}
