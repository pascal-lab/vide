use rustc_hash::FxHashMap;

use crate::{
    EntityId, FilePosition, FileRange, OriginId, SourceBlock, SourceBlockReason, SourceChoice,
    SourceContext, SourceContextId, SourceDomain, SourceDomainId, SourceEntity, SourceOrigin,
    SourcePurpose, SourceRangeResult, SourceRelation, SourceSelection, SourceSelectionId, Span,
    SpanId, relation::ResolutionReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGraph {
    domains: Vec<SourceDomain>,
    spans: Vec<Span>,
    selections: Vec<SourceSelection>,
    entities: Vec<SourceEntity>,
    contexts: Vec<SourceContext>,
    origins: Vec<SourceOrigin>,
    relations: Vec<SourceRelation>,
    selection_by_entity: FxHashMap<EntityId, SourceSelectionId>,
    origin_by_entity: FxHashMap<EntityId, OriginId>,
    written_origin_by_span: FxHashMap<SpanId, OriginId>,
    children_by_entity: FxHashMap<EntityId, Vec<EntityId>>,
    parents_by_entity: FxHashMap<EntityId, Vec<EntityId>>,
    resolutions_by_reference:
        FxHashMap<(SourceContextId, EntityId), Vec<(EntityId, ResolutionReason)>>,
    references_by_definition:
        FxHashMap<(SourceContextId, EntityId), Vec<(EntityId, ResolutionReason)>>,
    includes_by_directive: FxHashMap<(SourceContextId, crate::IncludeDirectiveId), SourceContextId>,
    expansions_by_call: FxHashMap<(SourceContextId, crate::MacroCallId), crate::MacroExpansionId>,
    tokens_by_expansion: FxHashMap<crate::MacroExpansionId, Vec<EntityId>>,
    spellings_by_generated: FxHashMap<SpanId, Vec<(SpanId, crate::SpellingKind)>>,
    generated_by_spelling_source: FxHashMap<SpanId, Vec<(SpanId, crate::SpellingKind)>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceGraphBuilder {
    domains: Vec<SourceDomain>,
    domain_ids: FxHashMap<SourceDomain, SourceDomainId>,
    spans: Vec<Span>,
    span_ids: FxHashMap<Span, SpanId>,
    selections: Vec<SourceSelection>,
    selection_ids: FxHashMap<SourceSelection, SourceSelectionId>,
    entities: Vec<SourceEntity>,
    contexts: Vec<SourceContext>,
    origins: Vec<SourceOrigin>,
    origin_ids: FxHashMap<SourceOrigin, OriginId>,
    relations: Vec<SourceRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityHit {
    pub entity: EntityId,
    pub selection: SourceSelectionId,
    pub matched_span: SpanId,
}

impl SourceGraphBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern_domain(&mut self, domain: SourceDomain) -> SourceDomainId {
        if let Some(id) = self.domain_ids.get(&domain).copied() {
            return id;
        }

        let id = SourceDomainId::new(self.domains.len() as u32);
        self.domains.push(domain.clone());
        self.domain_ids.insert(domain, id);
        id
    }

    pub fn intern_span(
        &mut self,
        domain: SourceDomainId,
        range: utils::line_index::TextRange,
    ) -> SpanId {
        let span = Span { domain, range };
        if let Some(id) = self.span_ids.get(&span).copied() {
            return id;
        }

        let id = SpanId::new(self.spans.len() as u32);
        self.spans.push(span);
        self.span_ids.insert(span, id);
        id
    }

    pub fn intern_selection(&mut self, full: SpanId, focus: Option<SpanId>) -> SourceSelectionId {
        let selection = SourceSelection { full, focus };
        if let Some(id) = self.selection_ids.get(&selection).copied() {
            return id;
        }

        let id = SourceSelectionId::new(self.selections.len() as u32);
        self.selections.push(selection);
        self.selection_ids.insert(selection, id);
        id
    }

    pub fn add_entity(&mut self, entity: SourceEntity) -> EntityId {
        let id = EntityId::new(self.entities.len() as u32);
        self.entities.push(entity);
        id
    }

    pub fn add_context(&mut self, context: SourceContext) -> SourceContextId {
        let id = SourceContextId::new(self.contexts.len() as u32);
        self.contexts.push(context);
        id
    }

    pub fn add_origin(&mut self, origin: SourceOrigin) -> OriginId {
        if let Some(id) = self.origin_ids.get(&origin).copied() {
            return id;
        }

        let id = OriginId::new(self.origins.len() as u32);
        self.origins.push(origin.clone());
        self.origin_ids.insert(origin, id);
        id
    }

    pub fn add_written_origin(&mut self, span: SpanId) -> OriginId {
        self.add_origin(SourceOrigin::Written { span })
    }

    pub fn add_relation(&mut self, relation: SourceRelation) {
        self.relations.push(relation);
    }

    pub fn build(self) -> SourceGraph {
        let mut selection_by_entity = FxHashMap::default();
        let mut origin_by_entity = FxHashMap::default();
        let mut written_origin_by_span = FxHashMap::default();
        let mut children_by_entity: FxHashMap<EntityId, Vec<EntityId>> = FxHashMap::default();
        let mut parents_by_entity: FxHashMap<EntityId, Vec<EntityId>> = FxHashMap::default();
        let mut resolutions_by_reference: FxHashMap<
            (SourceContextId, EntityId),
            Vec<(EntityId, ResolutionReason)>,
        > = FxHashMap::default();
        let mut references_by_definition: FxHashMap<
            (SourceContextId, EntityId),
            Vec<(EntityId, ResolutionReason)>,
        > = FxHashMap::default();
        let mut includes_by_directive = FxHashMap::default();
        let mut expansions_by_call = FxHashMap::default();
        let mut tokens_by_expansion: FxHashMap<crate::MacroExpansionId, Vec<EntityId>> =
            FxHashMap::default();
        let mut spellings_by_generated: FxHashMap<SpanId, Vec<(SpanId, crate::SpellingKind)>> =
            FxHashMap::default();
        let mut generated_by_spelling_source: FxHashMap<
            SpanId,
            Vec<(SpanId, crate::SpellingKind)>,
        > = FxHashMap::default();

        for relation in &self.relations {
            match *relation {
                SourceRelation::Contains { parent, child } => {
                    children_by_entity.entry(parent).or_default().push(child);
                    parents_by_entity.entry(child).or_default().push(parent);
                }
                SourceRelation::HasSelection { entity, selection } => {
                    selection_by_entity.insert(entity, selection);
                }
                SourceRelation::HasOrigin { entity, origin } => {
                    origin_by_entity.insert(entity, origin);
                }
                SourceRelation::ResolvesTo { context, reference, definition, reason } => {
                    resolutions_by_reference
                        .entry((context, reference))
                        .or_default()
                        .push((definition, reason));
                    references_by_definition
                        .entry((context, definition))
                        .or_default()
                        .push((reference, reason));
                }
                SourceRelation::Includes { context, directive, included_context } => {
                    includes_by_directive.insert((context, directive), included_context);
                }
                SourceRelation::Expands { context, call, expansion } => {
                    expansions_by_call.insert((context, call), expansion);
                }
                SourceRelation::EmitsToken { expansion, token } => {
                    tokens_by_expansion.entry(expansion).or_default().push(token);
                }
                SourceRelation::SpelledFrom { generated, source, kind } => {
                    spellings_by_generated.entry(generated).or_default().push((source, kind));
                    generated_by_spelling_source.entry(source).or_default().push((generated, kind));
                }
                _ => {}
            }
        }
        for (raw, origin) in self.origins.iter().enumerate() {
            if let SourceOrigin::Written { span } = *origin {
                written_origin_by_span.insert(span, OriginId::new(raw as u32));
            }
        }

        SourceGraph {
            domains: self.domains,
            spans: self.spans,
            selections: self.selections,
            entities: self.entities,
            contexts: self.contexts,
            origins: self.origins,
            relations: self.relations,
            selection_by_entity,
            origin_by_entity,
            written_origin_by_span,
            children_by_entity,
            parents_by_entity,
            resolutions_by_reference,
            references_by_definition,
            includes_by_directive,
            expansions_by_call,
            tokens_by_expansion,
            spellings_by_generated,
            generated_by_spelling_source,
        }
    }
}

impl SourceGraph {
    pub fn domain(&self, id: SourceDomainId) -> &SourceDomain {
        &self.domains[id.raw() as usize]
    }

    pub fn span(&self, id: SpanId) -> Span {
        self.spans[id.raw() as usize]
    }

    pub fn selection(&self, id: SourceSelectionId) -> SourceSelection {
        self.selections[id.raw() as usize]
    }

    pub fn entity(&self, id: EntityId) -> SourceEntity {
        self.entities[id.raw() as usize]
    }

    pub fn context(&self, id: SourceContextId) -> &SourceContext {
        &self.contexts[id.raw() as usize]
    }

    pub fn origin(&self, id: OriginId) -> &SourceOrigin {
        &self.origins[id.raw() as usize]
    }

    pub fn relations(&self) -> &[SourceRelation] {
        &self.relations
    }

    pub fn entity_selection(&self, entity: EntityId) -> Option<SourceSelectionId> {
        self.selection_by_entity.get(&entity).copied()
    }

    pub fn entity_origin(&self, entity: EntityId) -> Option<OriginId> {
        self.origin_by_entity.get(&entity).copied()
    }

    pub fn entity_children(&self, entity: EntityId) -> &[EntityId] {
        self.children_by_entity.get(&entity).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn entity_parents(&self, entity: EntityId) -> &[EntityId] {
        self.parents_by_entity.get(&entity).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn entity_focus_file_range(
        &self,
        entity: EntityId,
        purpose: SourcePurpose,
    ) -> SourceRangeResult<FileRange> {
        let Some(selection) = self.entity_selection(entity) else {
            return SourceRangeResult::Unavailable(crate::SourceUnavailable::Unsupported);
        };
        let selection = self.selection(selection);
        let focus = selection.focus.unwrap_or(selection.full);
        match self.to_file_range(focus, purpose) {
            SourceRangeResult::Mapped(range) => SourceRangeResult::Mapped(range),
            SourceRangeResult::Blocked(_) | SourceRangeResult::Unavailable(_) => {
                self.to_file_range(selection.full, purpose)
            }
        }
    }

    pub fn entity_full_file_range(
        &self,
        entity: EntityId,
        purpose: SourcePurpose,
    ) -> SourceRangeResult<FileRange> {
        let Some(selection) = self.entity_selection(entity) else {
            return SourceRangeResult::Unavailable(crate::SourceUnavailable::Unsupported);
        };
        self.to_file_range(self.selection(selection).full, purpose)
    }

    pub fn resolved_definitions(
        &self,
        context: SourceContextId,
        reference: EntityId,
    ) -> &[(EntityId, ResolutionReason)] {
        self.resolutions_by_reference.get(&(context, reference)).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn resolved_references(
        &self,
        context: SourceContextId,
        definition: EntityId,
    ) -> &[(EntityId, ResolutionReason)] {
        self.references_by_definition.get(&(context, definition)).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn included_context(
        &self,
        context: SourceContextId,
        directive: crate::IncludeDirectiveId,
    ) -> Option<SourceContextId> {
        self.includes_by_directive.get(&(context, directive)).copied()
    }

    pub fn expansion_for_call(
        &self,
        context: SourceContextId,
        call: crate::MacroCallId,
    ) -> Option<crate::MacroExpansionId> {
        self.expansions_by_call.get(&(context, call)).copied()
    }

    pub fn emitted_tokens(&self, expansion: crate::MacroExpansionId) -> &[EntityId] {
        self.tokens_by_expansion.get(&expansion).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn spelled_sources(&self, generated: SpanId) -> &[(SpanId, crate::SpellingKind)] {
        self.spellings_by_generated.get(&generated).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn generated_from_spelling_source(
        &self,
        source: SpanId,
    ) -> &[(SpanId, crate::SpellingKind)] {
        self.generated_by_spelling_source.get(&source).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn generated_from_file_position(
        &self,
        position: FilePosition,
    ) -> Vec<(SpanId, crate::SpellingKind)> {
        self.generated_spelling_hits_for_file_position(position)
            .into_iter()
            .map(|(_, generated, kind)| (generated, kind))
            .collect()
    }

    pub fn generated_spelling_hits_for_file_position(
        &self,
        position: FilePosition,
    ) -> Vec<(SpanId, SpanId, crate::SpellingKind)> {
        let mut generated = Vec::new();
        for (source, targets) in &self.generated_by_spelling_source {
            let source_span = self.span(*source);
            if self.file_id_for_domain(source_span.domain) == Some(position.file_id)
                && source_span.range.contains(position.offset)
            {
                generated
                    .extend(targets.iter().map(|(generated, kind)| (*source, *generated, *kind)));
            }
        }
        generated
    }

    pub fn entities_at_file_position(
        &self,
        position: FilePosition,
        _context: Option<SourceContextId>,
    ) -> Vec<EntityHit> {
        let mut hits = Vec::new();
        for (raw, _) in self.entities.iter().enumerate() {
            let entity = EntityId::new(raw as u32);
            let Some(selection) = self.entity_selection(entity) else {
                continue;
            };
            let selection_data = self.selection(selection);
            let span = selection_data.focus.unwrap_or(selection_data.full);
            let span_data = self.span(span);
            if self.file_id_for_domain(span_data.domain) == Some(position.file_id)
                && span_data.range.contains(position.offset)
            {
                hits.push(EntityHit { entity, selection, matched_span: span });
            }
        }
        hits
    }

    pub fn entities_intersecting_file_range(
        &self,
        file_id: vfs::FileId,
        range: utils::line_index::TextRange,
        _context: Option<SourceContextId>,
    ) -> Vec<EntityHit> {
        let mut hits = Vec::new();
        for (raw, _) in self.entities.iter().enumerate() {
            let entity = EntityId::new(raw as u32);
            let Some(selection) = self.entity_selection(entity) else {
                continue;
            };
            let selection_data = self.selection(selection);
            let span = selection_data.focus.unwrap_or(selection_data.full);
            let span_data = self.span(span);
            if self.file_id_for_domain(span_data.domain) == Some(file_id)
                && span_data.range.intersect(range).is_some()
            {
                hits.push(EntityHit { entity, selection, matched_span: span });
            }
        }
        hits
    }

    pub fn origins_for_span(&self, span: SpanId) -> Vec<OriginId> {
        self.origins
            .iter()
            .enumerate()
            .filter_map(|(raw, origin)| {
                origin_mentions_span(origin, span).then(|| OriginId::new(raw as u32))
            })
            .collect()
    }

    pub fn written_origin_for_span(&self, span: SpanId) -> Option<OriginId> {
        self.written_origin_by_span.get(&span).copied()
    }

    pub fn written_origin_for_file_range(&self, range: FileRange) -> Option<OriginId> {
        self.spans.iter().enumerate().find_map(|(raw, span)| {
            if span.range != range.range
                || self.file_id_for_domain(span.domain) != Some(range.file_id)
            {
                return None;
            }
            self.written_origin_for_span(SpanId::new(raw as u32))
        })
    }

    pub fn written_origins_for_file(
        &self,
        file_id: vfs::FileId,
    ) -> impl Iterator<Item = (FileRange, OriginId)> + '_ {
        self.written_origin_by_span.iter().filter_map(move |(span, origin)| {
            let span = self.span(*span);
            (self.file_id_for_domain(span.domain) == Some(file_id))
                .then_some((FileRange { file_id, range: span.range }, *origin))
        })
    }

    pub fn lowering_origins_for_file(&self, file_id: vfs::FileId) -> Vec<(FileRange, OriginId)> {
        let mut origins = self.written_origins_for_file(file_id).collect::<Vec<_>>();
        for (entity, origin) in &self.origin_by_entity {
            let Some(selection) = self.entity_selection(*entity) else {
                continue;
            };
            let span = self.span(self.selection(selection).full);
            if self.file_id_for_domain(span.domain) == Some(file_id) {
                origins.push((FileRange { file_id, range: span.range }, *origin));
            }
        }
        origins
    }

    pub fn preferred_span(&self, origin: OriginId, purpose: SourcePurpose) -> SourceChoice {
        match self.origin(origin) {
            SourceOrigin::Written { span } => SourceChoice::Span(*span),
            SourceOrigin::MacroBody { body_span, call_span, emitted_span, .. } => match purpose {
                SourcePurpose::GotoDefinition => SourceChoice::Span(*body_span),
                SourcePurpose::Hover | SourcePurpose::Completion | SourcePurpose::CodeAction => {
                    SourceChoice::Span(*emitted_span)
                }
                SourcePurpose::FindReferences
                | SourcePurpose::Rename
                | SourcePurpose::Diagnostic
                | SourcePurpose::SemanticToken => SourceChoice::Span(*call_span),
            },
            SourceOrigin::MacroArgument {
                argument_span, body_param_span, emitted_span, ..
            } => match purpose {
                SourcePurpose::GotoDefinition => SourceChoice::Span(*body_param_span),
                SourcePurpose::Hover | SourcePurpose::Completion | SourcePurpose::CodeAction => {
                    SourceChoice::Span(*emitted_span)
                }
                SourcePurpose::FindReferences
                | SourcePurpose::Rename
                | SourcePurpose::Diagnostic
                | SourcePurpose::SemanticToken => SourceChoice::Span(*argument_span),
            },
            SourceOrigin::TokenPaste { call_span, emitted_span, .. }
            | SourceOrigin::Stringification { call_span, emitted_span, .. } => match purpose {
                SourcePurpose::Hover | SourcePurpose::Completion | SourcePurpose::CodeAction => {
                    SourceChoice::Span(*emitted_span)
                }
                SourcePurpose::GotoDefinition
                | SourcePurpose::FindReferences
                | SourcePurpose::Rename
                | SourcePurpose::Diagnostic
                | SourcePurpose::SemanticToken => SourceChoice::Span(*call_span),
            },
            SourceOrigin::Builtin { call_span, emitted_span, .. } => match purpose {
                SourcePurpose::Hover | SourcePurpose::Completion | SourcePurpose::CodeAction => {
                    SourceChoice::Span(*emitted_span)
                }
                SourcePurpose::GotoDefinition
                | SourcePurpose::FindReferences
                | SourcePurpose::Rename
                | SourcePurpose::Diagnostic
                | SourcePurpose::SemanticToken => SourceChoice::Span(*call_span),
            },
            SourceOrigin::Synthetic { preferred_span, .. } => {
                preferred_span.map(SourceChoice::Span).unwrap_or(SourceChoice::Unavailable)
            }
            SourceOrigin::Composite { preferred_span, .. } => {
                preferred_span.map(SourceChoice::Span).unwrap_or(SourceChoice::Unavailable)
            }
            SourceOrigin::Unavailable { .. } => SourceChoice::Unavailable,
            SourceOrigin::Alias { origin } => self.preferred_span(*origin, purpose),
        }
    }

    pub fn to_file_range(
        &self,
        span: SpanId,
        _purpose: SourcePurpose,
    ) -> SourceRangeResult<FileRange> {
        let span_data = self.span(span);
        match self.domain(span_data.domain) {
            SourceDomain::RealFile { file_id } | SourceDomain::VirtualFile { file_id, .. } => {
                SourceRangeResult::Mapped(FileRange { file_id: *file_id, range: span_data.range })
            }
            SourceDomain::Unmapped { reason } => SourceRangeResult::Unavailable(reason.clone()),
            SourceDomain::VirtualDisplay { .. }
            | SourceDomain::SlangSourceBuffer { .. }
            | SourceDomain::ExpansionDisplay { .. }
            | SourceDomain::ExpansionParseBuffer { .. }
            | SourceDomain::Builtin { .. } => SourceRangeResult::Blocked(SourceBlock {
                reason: SourceBlockReason::DisplayOnly,
                preferred_span: Some(span),
            }),
        }
    }

    fn file_id_for_domain(&self, domain: SourceDomainId) -> Option<vfs::FileId> {
        match self.domain(domain) {
            SourceDomain::RealFile { file_id } | SourceDomain::VirtualFile { file_id, .. } => {
                Some(*file_id)
            }
            SourceDomain::VirtualDisplay { .. }
            | SourceDomain::SlangSourceBuffer { .. }
            | SourceDomain::ExpansionDisplay { .. }
            | SourceDomain::ExpansionParseBuffer { .. }
            | SourceDomain::Builtin { .. }
            | SourceDomain::Unmapped { .. } => None,
        }
    }
}

fn origin_mentions_span(origin: &SourceOrigin, span: SpanId) -> bool {
    match origin {
        SourceOrigin::Written { span: origin_span } => *origin_span == span,
        SourceOrigin::MacroBody { body_span, call_span, emitted_span, .. } => {
            [*body_span, *call_span, *emitted_span].contains(&span)
        }
        SourceOrigin::MacroArgument {
            argument_span,
            body_param_span,
            call_span,
            emitted_span,
            ..
        } => [*argument_span, *body_param_span, *call_span, *emitted_span].contains(&span),
        SourceOrigin::TokenPaste { inputs, call_span, emitted_span, .. }
        | SourceOrigin::Stringification { inputs, call_span, emitted_span, .. } => {
            inputs.contains(&span) || [*call_span, *emitted_span].contains(&span)
        }
        SourceOrigin::Builtin { call_span, emitted_span, .. } => {
            [*call_span, *emitted_span].contains(&span)
        }
        SourceOrigin::Synthetic { preferred_span, .. }
        | SourceOrigin::Composite { preferred_span, .. } => *preferred_span == Some(span),
        SourceOrigin::Unavailable { .. } | SourceOrigin::Alias { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use utils::line_index::{TextRange, TextSize};
    use vfs::FileId;

    use super::*;
    use crate::{IncludeDirectiveId, MacroDefinitionId, MacroReferenceId, SourceUnavailable};

    #[test]
    fn interns_domains_spans_and_selections() {
        let mut builder = SourceGraphBuilder::new();
        let file = FileId(7);
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: file });
        let same_domain = builder.intern_domain(SourceDomain::RealFile { file_id: file });
        assert_eq!(domain, same_domain);

        let range = TextRange::new(TextSize::from(1), TextSize::from(5));
        let span = builder.intern_span(domain, range);
        let same_span = builder.intern_span(domain, range);
        assert_eq!(span, same_span);

        let selection = builder.intern_selection(span, Some(span));
        let same_selection = builder.intern_selection(span, Some(span));
        assert_eq!(selection, same_selection);
    }

    #[test]
    fn resolves_entity_selection_and_hits_file_position() {
        let mut builder = SourceGraphBuilder::new();
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let full = builder.intern_span(domain, TextRange::new(0.into(), 12.into()));
        let focus = builder.intern_span(domain, TextRange::new(8.into(), 11.into()));
        let selection = builder.intern_selection(full, Some(focus));
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });

        let graph = builder.build();

        assert_eq!(graph.entity_selection(entity), Some(selection));
        let hits = graph.entities_at_file_position(
            FilePosition { file_id: FileId(1), offset: TextSize::from(9) },
            None,
        );
        assert_eq!(hits, vec![EntityHit { entity, selection, matched_span: focus }]);
    }

    #[test]
    fn entity_hit_testing_uses_focus_when_available() {
        let mut builder = SourceGraphBuilder::new();
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let full = builder.intern_span(domain, TextRange::new(0.into(), 12.into()));
        let focus = builder.intern_span(domain, TextRange::new(8.into(), 11.into()));
        let selection = builder.intern_selection(full, Some(focus));
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });

        let graph = builder.build();

        assert_eq!(
            graph.entities_at_file_position(
                FilePosition { file_id: FileId(1), offset: TextSize::from(9) },
                None,
            ),
            vec![EntityHit { entity, selection, matched_span: focus }]
        );
        assert!(
            graph
                .entities_at_file_position(
                    FilePosition { file_id: FileId(1), offset: TextSize::from(2) },
                    None,
                )
                .is_empty()
        );
    }

    #[test]
    fn resolves_entity_selection_hits_file_range() {
        let mut builder = SourceGraphBuilder::new();
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let full = builder.intern_span(domain, TextRange::new(8.into(), 20.into()));
        let focus = builder.intern_span(domain, TextRange::new(10.into(), 14.into()));
        let selection = builder.intern_selection(full, Some(focus));
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });

        let graph = builder.build();

        let hits = graph.entities_intersecting_file_range(
            FileId(1),
            TextRange::new(12.into(), 16.into()),
            None,
        );
        assert_eq!(hits, vec![EntityHit { entity, selection, matched_span: focus }]);
    }

    #[test]
    fn indexes_context_specific_resolution_relations() {
        let mut builder = SourceGraphBuilder::new();
        let context = builder
            .add_context(SourceContext::CompilationRoot { profile_id: None, root_file: FileId(1) });
        let reference = builder.add_entity(SourceEntity::MacroReference(MacroReferenceId::new(0)));
        let definition =
            builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(1)));
        builder.add_relation(SourceRelation::ResolvesTo {
            context,
            reference,
            definition,
            reason: ResolutionReason::VisibleDefinition,
        });

        let graph = builder.build();

        assert_eq!(
            graph.resolved_definitions(context, reference),
            &[(definition, ResolutionReason::VisibleDefinition)]
        );
        assert_eq!(
            graph.resolved_references(context, definition),
            &[(reference, ResolutionReason::VisibleDefinition)]
        );
    }

    #[test]
    fn indexes_contains_relations() {
        let mut builder = SourceGraphBuilder::new();
        let parent = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        let child = builder.add_entity(SourceEntity::MacroReference(MacroReferenceId::new(1)));
        builder.add_relation(SourceRelation::Contains { parent, child });

        let graph = builder.build();

        assert_eq!(graph.entity_children(parent), &[child]);
        assert_eq!(graph.entity_parents(child), &[parent]);
        assert!(graph.entity_children(child).is_empty());
        assert!(graph.entity_parents(parent).is_empty());
    }

    #[test]
    fn projects_entity_focus_to_file_range() {
        let mut builder = SourceGraphBuilder::new();
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let full = builder.intern_span(domain, TextRange::new(0.into(), 12.into()));
        let focus = builder.intern_span(domain, TextRange::new(8.into(), 11.into()));
        let selection = builder.intern_selection(full, Some(focus));
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });

        let graph = builder.build();

        assert_eq!(
            graph.entity_focus_file_range(entity, SourcePurpose::GotoDefinition),
            SourceRangeResult::Mapped(FileRange {
                file_id: FileId(1),
                range: TextRange::new(8.into(), 11.into()),
            })
        );
    }

    #[test]
    fn projects_entity_full_to_file_range() {
        let mut builder = SourceGraphBuilder::new();
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let full = builder.intern_span(domain, TextRange::new(0.into(), 12.into()));
        let focus = builder.intern_span(domain, TextRange::new(8.into(), 11.into()));
        let selection = builder.intern_selection(full, Some(focus));
        let entity = builder.add_entity(SourceEntity::MacroDefinition(MacroDefinitionId::new(0)));
        builder.add_relation(SourceRelation::HasSelection { entity, selection });

        let graph = builder.build();

        assert_eq!(
            graph.entity_full_file_range(entity, SourcePurpose::GotoDefinition),
            SourceRangeResult::Mapped(FileRange {
                file_id: FileId(1),
                range: TextRange::new(0.into(), 12.into()),
            })
        );
    }

    #[test]
    fn indexes_context_specific_include_relations() {
        let mut builder = SourceGraphBuilder::new();
        let root_context = builder
            .add_context(SourceContext::CompilationRoot { profile_id: None, root_file: FileId(1) });
        let included_context = builder.add_context(SourceContext::IncludeContext {
            parent: root_context,
            include_directive: IncludeDirectiveId::new(3),
            included_file: FileId(2),
        });
        builder.add_relation(SourceRelation::Includes {
            context: root_context,
            directive: IncludeDirectiveId::new(3),
            included_context,
        });

        let graph = builder.build();

        assert_eq!(
            graph.included_context(root_context, IncludeDirectiveId::new(3)),
            Some(included_context)
        );
    }

    #[test]
    fn indexes_expansion_token_and_spelling_relations() {
        let mut builder = SourceGraphBuilder::new();
        let context = builder
            .add_context(SourceContext::CompilationRoot { profile_id: None, root_file: FileId(1) });
        let expansion = crate::MacroExpansionId::new(2);
        let call = crate::MacroCallId::new(3);
        let token =
            builder.add_entity(SourceEntity::ExpansionToken(crate::ExpansionTokenId::new(4)));
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(1) });
        let generated = builder.intern_span(domain, TextRange::new(10.into(), 15.into()));
        let source = builder.intern_span(domain, TextRange::new(2.into(), 7.into()));

        builder.add_relation(SourceRelation::Expands { context, call, expansion });
        builder.add_relation(SourceRelation::EmitsToken { expansion, token });
        builder.add_relation(SourceRelation::SpelledFrom {
            generated,
            source,
            kind: crate::SpellingKind::MacroArgument,
        });

        let graph = builder.build();

        assert_eq!(graph.expansion_for_call(context, call), Some(expansion));
        assert_eq!(graph.emitted_tokens(expansion), &[token]);
        assert_eq!(
            graph.spelled_sources(generated),
            &[(source, crate::SpellingKind::MacroArgument)]
        );
        assert_eq!(
            graph.generated_from_spelling_source(source),
            &[(generated, crate::SpellingKind::MacroArgument)]
        );
        assert_eq!(
            graph.generated_from_file_position(FilePosition {
                file_id: FileId(1),
                offset: TextSize::from(3),
            }),
            vec![(generated, crate::SpellingKind::MacroArgument)]
        );
        assert!(
            graph
                .generated_from_file_position(FilePosition {
                    file_id: FileId(1),
                    offset: TextSize::from(8),
                })
                .is_empty()
        );
    }

    #[test]
    fn interns_written_origins_for_file_ranges() {
        let mut builder = SourceGraphBuilder::new();
        let file_id = FileId(2);
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id });
        let span = builder.intern_span(domain, TextRange::new(4.into(), 9.into()));
        let origin = builder.add_written_origin(span);
        let same_origin = builder.add_written_origin(span);
        let other_file_id = FileId(3);
        let other_domain = builder.intern_domain(SourceDomain::RealFile { file_id: other_file_id });
        let other_span = builder.intern_span(other_domain, TextRange::new(4.into(), 9.into()));
        let other_origin = builder.add_written_origin(other_span);
        let graph = builder.build();

        let file_range = FileRange { file_id, range: TextRange::new(4.into(), 9.into()) };
        let other_file_range =
            FileRange { file_id: other_file_id, range: TextRange::new(4.into(), 9.into()) };

        assert_eq!(origin, same_origin);
        assert_eq!(graph.written_origin_for_span(span), Some(origin));
        assert_eq!(graph.written_origin_for_file_range(file_range), Some(origin));
        assert_eq!(graph.written_origin_for_file_range(other_file_range), Some(other_origin));
        assert!(graph.lowering_origins_for_file(file_id).contains(&(file_range, origin)));
        assert!(
            !graph
                .lowering_origins_for_file(file_id)
                .iter()
                .any(|(range, _)| range.file_id == other_file_id)
        );
    }

    #[test]
    fn projects_only_file_backed_domains_to_file_ranges() {
        let mut builder = SourceGraphBuilder::new();
        let real = builder.intern_domain(SourceDomain::RealFile { file_id: FileId(3) });
        let unmapped = builder
            .intern_domain(SourceDomain::Unmapped { reason: SourceUnavailable::Unsupported });
        let real_span = builder.intern_span(real, TextRange::new(2.into(), 4.into()));
        let unmapped_span = builder.intern_span(unmapped, TextRange::new(0.into(), 1.into()));
        let graph = builder.build();

        assert_eq!(
            graph.to_file_range(real_span, SourcePurpose::Hover),
            SourceRangeResult::Mapped(FileRange {
                file_id: FileId(3),
                range: TextRange::new(2.into(), 4.into())
            })
        );
        assert_eq!(
            graph.to_file_range(unmapped_span, SourcePurpose::Hover),
            SourceRangeResult::Unavailable(SourceUnavailable::Unsupported)
        );
    }
}
