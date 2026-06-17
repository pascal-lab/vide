use super::*;

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn record_position_boundaries(&mut self) {
        self.tables.state_timeline.final_source_order = self.index.event_records.len();
        self.record_source_order_scopes();
        for (source_order, directive) in self.index.event_records.iter().enumerate() {
            self.tables
                .state_timeline
                .source_order_boundaries
                .entry(directive.range.source)
                .or_default()
                .push(SourceMacroStatePositionBoundary {
                    source_order,
                    boundary: boundary_after(directive.range),
                });
        }

        for boundaries in self.tables.state_timeline.source_order_boundaries.values_mut() {
            boundaries.sort_by_key(|boundary| (boundary.boundary.offset, boundary.source_order));
        }
    }

    pub(in crate::source::provenance::builder) fn record_source_order_scopes(&mut self) {
        let event_orders_by_id = self
            .index
            .event_records
            .iter()
            .enumerate()
            .map(|(source_order, directive)| (directive.event_id, source_order))
            .collect::<BTreeMap<_, _>>();
        let source_parents = self.source_parents_by_include();

        for source in &self.index.sources {
            let end_order = match source.origin {
                PreprocSourceOrigin::Root
                | PreprocSourceOrigin::Predefine
                | PreprocSourceOrigin::Detached => self.index.event_records.len(),
                PreprocSourceOrigin::Included { include_event_id } => {
                    let Some(include_order) = event_orders_by_id.get(&include_event_id).copied()
                    else {
                        continue;
                    };
                    self.included_source_end_order(source.id, include_order, &source_parents)
                }
            };
            self.tables
                .state_timeline
                .source_order_scopes
                .insert(source.id, SourceMacroStateSourceScope { end_order });
        }
    }

    pub(in crate::source::provenance::builder) fn source_parents_by_include(
        &self,
    ) -> BTreeMap<PreprocSourceId, PreprocSourceId> {
        let include_sources_by_event = self
            .index
            .event_records
            .iter()
            .map(|directive| (directive.event_id, directive.range.source))
            .collect::<BTreeMap<_, _>>();

        self.index
            .sources
            .iter()
            .filter_map(|source| match source.origin {
                PreprocSourceOrigin::Included { include_event_id } => include_sources_by_event
                    .get(&include_event_id)
                    .copied()
                    .map(|parent| (source.id, parent)),
                PreprocSourceOrigin::Root
                | PreprocSourceOrigin::Predefine
                | PreprocSourceOrigin::Detached => None,
            })
            .collect()
    }

    pub(in crate::source::provenance::builder) fn included_source_end_order(
        &self,
        source: PreprocSourceId,
        include_order: usize,
        source_parents: &BTreeMap<PreprocSourceId, PreprocSourceId>,
    ) -> usize {
        self.index
            .event_records
            .iter()
            .enumerate()
            .skip(include_order + 1)
            .find_map(|(source_order, directive)| {
                (!source_is_descendant_or_same(directive.range.source, source, source_parents))
                    .then_some(source_order)
            })
            .unwrap_or(self.index.event_records.len())
    }

    pub(in crate::source::provenance::builder) fn build_include_graph(&mut self) {
        self.tables.inactive_ranges = self.index.inactive_ranges.clone();
        let mut resolved_sources_by_event = BTreeMap::new();

        for edge in &self.index.include_edges {
            resolved_sources_by_event.insert(edge.include_event_id, edge.included_source);
        }

        for source in &self.index.sources {
            if source.origin == PreprocSourceOrigin::Detached {
                self.include_edges_partial = true;
                self.tables
                    .issues
                    .push(SourcePreprocFactIssue::DetachedSource { source: source.id });
            }
        }

        self.tables.include_graph.edges = self.index.include_edges.clone();
        for include in &self.index.includes {
            let id = SourceIncludeDirectiveId::new(self.tables.include_graph.directives.len());
            let resolved_source = resolved_sources_by_event.get(&include.event_id).copied();
            let status = match resolved_source {
                Some(source) => SourceIncludeStatus::Resolved { source },
                None => SourceIncludeStatus::Unresolved,
            };
            self.tables.include_graph.directives.push(SourceIncludeDirective {
                id,
                event_id: include.event_id,
                directive_range: include.range,
                target: include.target.clone(),
                target_range: include.target_range,
                resolved_source,
                status,
            });
        }
    }
}

pub(in crate::source::provenance::builder) fn source_is_descendant_or_same(
    mut source: PreprocSourceId,
    ancestor: PreprocSourceId,
    source_parents: &BTreeMap<PreprocSourceId, PreprocSourceId>,
) -> bool {
    loop {
        if source == ancestor {
            return true;
        }
        let Some(parent) = source_parents.get(&source).copied() else {
            return false;
        };
        source = parent;
    }
}
