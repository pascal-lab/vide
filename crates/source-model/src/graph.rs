use rustc_hash::FxHashMap;

use crate::{
    EntityId, FilePosition, FileRange, OriginId, SourceBlock, SourceBlockReason, SourceChoice,
    SourceContext, SourceContextId, SourceDomain, SourceDomainId, SourceEntity, SourceOrigin,
    SourcePurpose, SourceRangeResult, SourceRelation, SourceSelection, SourceSelectionId, Span,
    SpanId,
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

        for relation in &self.relations {
            match *relation {
                SourceRelation::HasSelection { entity, selection } => {
                    selection_by_entity.insert(entity, selection);
                }
                SourceRelation::HasOrigin { entity, origin } => {
                    origin_by_entity.insert(entity, origin);
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
            for span in [selection_data.focus, Some(selection_data.full)].into_iter().flatten() {
                let span_data = self.span(span);
                if self.file_id_for_domain(span_data.domain) == Some(position.file_id)
                    && span_data.range.contains(position.offset)
                {
                    hits.push(EntityHit { entity, selection, matched_span: span });
                    break;
                }
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
        SourceOrigin::Synthetic { preferred_span, .. } => *preferred_span == Some(span),
        SourceOrigin::Unavailable { .. } | SourceOrigin::Alias { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use utils::line_index::{TextRange, TextSize};
    use vfs::FileId;

    use super::*;
    use crate::{MacroDefinitionId, SourceUnavailable};

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
    fn interns_written_origins_for_file_ranges() {
        let mut builder = SourceGraphBuilder::new();
        let file_id = FileId(2);
        let domain = builder.intern_domain(SourceDomain::RealFile { file_id });
        let span = builder.intern_span(domain, TextRange::new(4.into(), 9.into()));
        let origin = builder.add_written_origin(span);
        let same_origin = builder.add_written_origin(span);
        let graph = builder.build();

        assert_eq!(origin, same_origin);
        assert_eq!(graph.written_origin_for_span(span), Some(origin));
        assert_eq!(
            graph.written_origin_for_file_range(FileRange {
                file_id,
                range: TextRange::new(4.into(), 9.into()),
            }),
            Some(origin)
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
