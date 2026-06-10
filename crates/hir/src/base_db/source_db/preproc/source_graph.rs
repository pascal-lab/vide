use preproc::source::{
    PreprocSourceId, SourceEmittedTokenId, SourceEmittedTokenRange, SourceIncludeDirectiveId,
    SourceIncludeStatus, SourceMacroArgumentIdentity, SourceMacroBodyIdentity,
    SourceMacroOperationIdentity, SourceMacroResolution, SourceRange, SourceTokenProvenance,
};
use rustc_hash::FxHashMap;
use source_model::{
    EntityId, ExpansionTokenId, InactiveRegionId, IncludeDirectiveId, MacroArgumentTokenIdentity,
    MacroBodyTokenIdentity, MacroCallId, MacroCallIdentity, MacroDefinitionId,
    MacroDefinitionIdentity, MacroExpansionId, MacroExpansionIdentity, MacroOperationTokenIdentity,
    MacroReferenceId, ResolutionReason, SourceContext, SourceContextId, SourceDomain,
    SourceDomainId, SourceEntity, SourceGraph, SourceGraphBuilder, SourceOrigin, SourceRelation,
    SourceSelectionId, SourceUnavailable, SpanId, SpellingKind, SyntheticReason, VirtualOrigin,
};
use syntax::{SyntaxElement, SyntaxNode, has_text_range::HasTextRange};
use utils::line_index::{TextRange, TextSize};
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
    pub emitted_token_entities: FxHashMap<SourceEmittedTokenId, EntityId>,
}

pub(super) fn source_graph_preproc_model_from_mapped(
    mapped: &MappedSourcePreprocModel,
    root_file: FileId,
    profile_id: Option<CompilationProfileId>,
    root_syntax: Option<SyntaxNode<'_>>,
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
    if let Some(root_syntax) = root_syntax
        && let Some(root_source) = mapped.model.root_source()
        && let Some(root_domain) = source_domains.get(&root_source).copied()
    {
        add_written_syntax_origins(&mut builder, root_syntax, root_domain);
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

    let emitted_token_entities = add_preproc_entities(
        &mut builder,
        mapped,
        root_context,
        &source_domains,
        &include_contexts,
    );

    SourceGraphPreprocModel {
        graph: builder.build(),
        root_context,
        source_domains,
        include_contexts,
        emitted_token_entities,
    }
}

fn add_preproc_entities(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    root_context: SourceContextId,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    include_contexts: &FxHashMap<SourceIncludeDirectiveId, SourceContextId>,
) -> FxHashMap<SourceEmittedTokenId, EntityId> {
    let mut macro_def_entities = FxHashMap::default();
    let mut macro_ref_entities = FxHashMap::default();
    let mut macro_call_entities = FxHashMap::default();
    let mut emitted_token_entities = FxHashMap::default();
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
        add_expansion_token_entities(
            builder,
            mapped,
            source_domains,
            expansion.id,
            expansion.emitted_token_range,
            &mut emitted_token_entities,
        );
    }

    emitted_token_entities
}

fn add_written_syntax_origins(
    builder: &mut SourceGraphBuilder,
    root: SyntaxNode<'_>,
    domain: SourceDomainId,
) {
    add_written_node_origin(builder, root, domain);
    for child in root.children() {
        add_written_element_origins(builder, child, domain);
    }
}

fn add_written_element_origins(
    builder: &mut SourceGraphBuilder,
    element: SyntaxElement<'_>,
    domain: SourceDomainId,
) {
    if let Some(range) = element.text_range() {
        let span = builder.intern_span(domain, range);
        builder.add_written_origin(span);
    }
    if let Some(node) = element.as_node() {
        for child in node.children() {
            add_written_element_origins(builder, child, domain);
        }
    }
}

fn add_written_node_origin(
    builder: &mut SourceGraphBuilder,
    node: SyntaxNode<'_>,
    domain: SourceDomainId,
) {
    if let Some(range) = node.text_range() {
        let span = builder.intern_span(domain, range);
        builder.add_written_origin(span);
    }
}

fn add_expansion_token_entities(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    expansion: preproc::source::SourceMacroExpansionId,
    emitted_range: SourceEmittedTokenRange,
    emitted_token_entities: &mut FxHashMap<SourceEmittedTokenId, EntityId>,
) {
    let graph_expansion = MacroExpansionId::new(expansion.raw() as u32);
    let display_domain =
        builder.intern_domain(SourceDomain::ExpansionDisplay { expansion: graph_expansion });

    for token_id in emitted_token_ids(emitted_range) {
        let Some(token) = mapped.model.emitted_tokens().get(token_id) else {
            continue;
        };
        let display_range = mapped
            .source_map
            .emitted_token_display_range(expansion, token_id)
            .unwrap_or_else(|_| TextRange::empty(TextSize::from(0)));
        let emitted_span = builder.intern_span(display_domain, display_range);
        let entity = builder
            .add_entity(SourceEntity::ExpansionToken(ExpansionTokenId::new(token_id.raw() as u32)));
        emitted_token_entities.insert(token_id, entity);

        let selection = builder.intern_selection(emitted_span, Some(emitted_span));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });
        builder
            .add_relation(SourceRelation::EmitsToken { expansion: graph_expansion, token: entity });

        let Some(provenance) = mapped.model.token_provenance().get(token.provenance) else {
            continue;
        };
        let origin = source_origin_from_token_provenance(
            builder,
            mapped,
            source_domains,
            provenance,
            emitted_span,
        );
        let origin = builder.add_origin(origin);
        builder.add_relation(SourceRelation::HasOrigin { entity, origin });
        add_spelling_relations(builder, mapped, source_domains, provenance, emitted_span);
    }
}

fn emitted_token_ids(range: SourceEmittedTokenRange) -> impl Iterator<Item = SourceEmittedTokenId> {
    let start = range.start.raw();
    (start..start + range.len).map(SourceEmittedTokenId::new)
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

fn source_origin_from_token_provenance(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    provenance: &SourceTokenProvenance,
    emitted_span: SpanId,
) -> SourceOrigin {
    match provenance {
        SourceTokenProvenance::Source { token_range } => SourceOrigin::Written {
            span: intern_span_for_source_range(builder, mapped, source_domains, *token_range),
        },
        SourceTokenProvenance::MacroBody { identity, body_token_range, call, .. } => {
            SourceOrigin::MacroBody {
                identity: macro_body_identity_from_preproc(*identity),
                body_span: intern_span_for_source_range(
                    builder,
                    mapped,
                    source_domains,
                    *body_token_range,
                ),
                call_span: call_span(builder, mapped, source_domains, *call),
                emitted_span,
            }
        }
        SourceTokenProvenance::MacroArgument {
            identity,
            body_token_range,
            argument_token_range,
            call,
            ..
        } => SourceOrigin::MacroArgument {
            identity: macro_argument_identity_from_preproc(*identity),
            argument_span: intern_span_for_source_range(
                builder,
                mapped,
                source_domains,
                *argument_token_range,
            ),
            body_param_span: intern_span_for_source_range(
                builder,
                mapped,
                source_domains,
                *body_token_range,
            ),
            call_span: call_span(builder, mapped, source_domains, *call),
            emitted_span,
        },
        SourceTokenProvenance::TokenPaste { identity, call, inputs } => SourceOrigin::TokenPaste {
            identity: macro_operation_identity_from_preproc(*identity),
            inputs: input_spans(builder, mapped, source_domains, inputs),
            call_span: call_span(builder, mapped, source_domains, *call),
            emitted_span,
        },
        SourceTokenProvenance::Stringification { identity, call, inputs } => {
            SourceOrigin::Stringification {
                identity: macro_operation_identity_from_preproc(*identity),
                inputs: input_spans(builder, mapped, source_domains, inputs),
                call_span: call_span(builder, mapped, source_domains, *call),
                emitted_span,
            }
        }
        SourceTokenProvenance::Predefine { .. } => SourceOrigin::Synthetic {
            reason: SyntheticReason::Other("predefine".into()),
            preferred_span: None,
        },
        SourceTokenProvenance::Builtin { name, call, .. } => SourceOrigin::Builtin {
            name: name.clone(),
            call_span: call_span(builder, mapped, source_domains, *call),
            emitted_span,
        },
        SourceTokenProvenance::Unavailable(reason) => {
            SourceOrigin::Unavailable { reason: source_unavailable_from_preproc(reason) }
        }
    }
}

fn add_spelling_relations(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    provenance: &SourceTokenProvenance,
    emitted_span: SpanId,
) {
    match provenance {
        SourceTokenProvenance::Source { token_range } => add_spelled_from(
            builder,
            mapped,
            source_domains,
            emitted_span,
            *token_range,
            SpellingKind::Direct,
        ),
        SourceTokenProvenance::MacroBody { body_token_range, .. } => add_spelled_from(
            builder,
            mapped,
            source_domains,
            emitted_span,
            *body_token_range,
            SpellingKind::MacroBody,
        ),
        SourceTokenProvenance::MacroArgument { argument_token_range, .. } => add_spelled_from(
            builder,
            mapped,
            source_domains,
            emitted_span,
            *argument_token_range,
            SpellingKind::MacroArgument,
        ),
        SourceTokenProvenance::TokenPaste { inputs, .. } => {
            for input in inputs {
                add_spelled_from(
                    builder,
                    mapped,
                    source_domains,
                    emitted_span,
                    *input,
                    SpellingKind::TokenPaste,
                );
            }
        }
        SourceTokenProvenance::Stringification { inputs, .. } => {
            for input in inputs {
                add_spelled_from(
                    builder,
                    mapped,
                    source_domains,
                    emitted_span,
                    *input,
                    SpellingKind::Stringification,
                );
            }
        }
        SourceTokenProvenance::Builtin { call, .. } => {
            let source = call_span(builder, mapped, source_domains, *call);
            builder.add_relation(SourceRelation::SpelledFrom {
                generated: emitted_span,
                source,
                kind: SpellingKind::Builtin,
            });
        }
        SourceTokenProvenance::Predefine { .. } | SourceTokenProvenance::Unavailable(_) => {}
    }
}

fn add_spelled_from(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    generated: SpanId,
    source_range: SourceRange,
    kind: SpellingKind,
) {
    let source = intern_span_for_source_range(builder, mapped, source_domains, source_range);
    builder.add_relation(SourceRelation::SpelledFrom { generated, source, kind });
}

fn input_spans(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    inputs: &[SourceRange],
) -> Vec<SpanId> {
    inputs
        .iter()
        .map(|range| intern_span_for_source_range(builder, mapped, source_domains, *range))
        .collect()
}

fn call_span(
    builder: &mut SourceGraphBuilder,
    mapped: &MappedSourcePreprocModel,
    source_domains: &FxHashMap<PreprocSourceId, SourceDomainId>,
    call: preproc::source::SourceMacroCallId,
) -> SpanId {
    let Some(call) = mapped.model.macro_calls().get(call) else {
        let domain = builder.intern_domain(SourceDomain::Unmapped {
            reason: SourceUnavailable::MissingSource { source: 0 },
        });
        return builder.intern_span(domain, TextRange::empty(TextSize::from(0)));
    };
    intern_span_for_source_range(builder, mapped, source_domains, call.call_range)
}

fn macro_body_identity_from_preproc(identity: SourceMacroBodyIdentity) -> MacroBodyTokenIdentity {
    MacroBodyTokenIdentity {
        call: MacroCallIdentity::new(identity.call.raw()),
        definition: MacroDefinitionIdentity::new(identity.definition.raw()),
        expansion: MacroExpansionIdentity::new(identity.expansion.raw()),
        parent_expansion: identity.parent_expansion.map(|id| MacroExpansionIdentity::new(id.raw())),
        body_token_index: identity.body_token_index,
    }
}

fn macro_argument_identity_from_preproc(
    identity: SourceMacroArgumentIdentity,
) -> MacroArgumentTokenIdentity {
    MacroArgumentTokenIdentity {
        call: MacroCallIdentity::new(identity.call.raw()),
        definition: MacroDefinitionIdentity::new(identity.definition.raw()),
        expansion: MacroExpansionIdentity::new(identity.expansion.raw()),
        parent_expansion: identity.parent_expansion.map(|id| MacroExpansionIdentity::new(id.raw())),
        body_token_index: identity.body_token_index,
        argument_index: identity.argument_index,
        argument_token_index: identity.argument_token_index,
    }
}

fn macro_operation_identity_from_preproc(
    identity: SourceMacroOperationIdentity,
) -> MacroOperationTokenIdentity {
    MacroOperationTokenIdentity {
        call: MacroCallIdentity::new(identity.call.raw()),
        definition: MacroDefinitionIdentity::new(identity.definition.raw()),
        expansion: MacroExpansionIdentity::new(identity.expansion.raw()),
        parent_expansion: identity.parent_expansion.map(|id| MacroExpansionIdentity::new(id.raw())),
        body_token_index: identity.body_token_index,
        argument_index: identity.argument_index,
        argument_token_index: identity.argument_token_index,
    }
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
