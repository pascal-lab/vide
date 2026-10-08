use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn record_position_boundaries(&mut self) {
        self.model.state_timeline.final_source_order = self.event_records.len();
        self.record_source_order_scopes();
        for (source_order, directive) in self.event_records.iter().enumerate() {
            self.model
                .state_timeline
                .source_order_boundaries
                .entry(directive.range.source)
                .or_default()
                .push(SourceMacroStatePositionBoundary {
                    source_order,
                    boundary: boundary_after(directive.range),
                });
        }

        for boundaries in self.model.state_timeline.source_order_boundaries.values_mut() {
            boundaries.sort_by_key(|boundary| (boundary.boundary.offset, boundary.source_order));
        }
    }

    pub(in crate::source::tables::builder) fn record_source_order_scopes(&mut self) {
        let event_orders_by_id = self
            .event_records
            .iter()
            .enumerate()
            .map(|(source_order, directive)| (directive.event_id, source_order))
            .collect::<BTreeMap<_, _>>();
        let source_parents = self.source_parents_by_include();

        // Depth of each source in the include forest. Root, predefine, and
        // detached sources have no parent and sit at depth 0.
        let mut depth = BTreeMap::<PreprocSourceId, usize>::new();
        for source in &self.model.sources {
            let source_id = source.id;
            if depth.contains_key(&source_id) {
                continue;
            }
            let mut chain = Vec::new();
            let mut current = source_id;
            loop {
                if depth.contains_key(&current) {
                    break;
                }
                match source_parents.get(&current) {
                    Some(&parent) => {
                        chain.push(current);
                        current = parent;
                    }
                    None => {
                        depth.insert(current, 0);
                        break;
                    }
                }
            }
            let base = depth[&current];
            for (offset, source_id) in chain.iter().rev().enumerate() {
                depth.insert(*source_id, base + offset + 1);
            }
        }

        // Every included source closes at `include_order + 1` when its subtree
        // is empty; the stack pass below overrides this for non-empty subtrees.
        let mut end_orders = BTreeMap::<PreprocSourceId, usize>::new();
        for source in &self.model.sources {
            if let PreprocSourceOrigin::Included { include_event_id } = source.origin {
                let Some(include_order) = event_orders_by_id.get(&include_event_id).copied() else {
                    continue;
                };
                end_orders.insert(source.id, include_order + 1);
            }
        }

        // The trace events are a depth-first traversal of the include forest,
        // so an included source's scope ends exactly when the stream returns to
        // a shallower source. A monotonic stack computes every end order in one
        // O(events) pass (the old scan was O(sources * events * depth)).
        let mut open = Vec::<PreprocSourceId>::new();
        for (source_order, event) in self.event_records.iter().enumerate() {
            let source = event.range.source;
            let source_depth = depth.get(&source).copied().unwrap_or(0);
            while let Some(&top) = open.last() {
                if top == source || depth[&top] < source_depth {
                    break;
                }
                end_orders.insert(top, source_order);
                open.pop();
            }
            if source_depth >= 1 && open.last() != Some(&source) {
                open.push(source);
            }
        }
        for source in open {
            end_orders.insert(source, self.event_records.len());
        }

        for source in &self.model.sources {
            let end_order = match source.origin {
                PreprocSourceOrigin::Root
                | PreprocSourceOrigin::Predefine
                | PreprocSourceOrigin::Detached => self.event_records.len(),
                PreprocSourceOrigin::Included { .. } => {
                    let Some(&end_order) = end_orders.get(&source.id) else {
                        continue;
                    };
                    end_order
                }
            };
            self.model
                .state_timeline
                .source_order_scopes
                .insert(source.id, SourceMacroStateSourceScope { end_order });
        }
    }

    pub(in crate::source::tables::builder) fn source_parents_by_include(
        &self,
    ) -> BTreeMap<PreprocSourceId, PreprocSourceId> {
        let include_sources_by_event = self
            .event_records
            .iter()
            .map(|directive| (directive.event_id, directive.range.source))
            .collect::<BTreeMap<_, _>>();

        self.model
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

    pub(in crate::source::tables::builder) fn build_include_graph(&mut self) {
        let mut resolved_sources_by_event = BTreeMap::new();

        for edge in &self.include_edges {
            resolved_sources_by_event.insert(edge.include_event_id, edge.included_source);
        }

        for include in &self.includes {
            let id = SourceIncludeDirectiveId::new(self.model.include_graph.directives.len());
            let resolved_source = resolved_sources_by_event.get(&include.event_id).copied();
            let status = match resolved_source {
                Some(source) => SourceIncludeStatus::Resolved { source },
                None => SourceIncludeStatus::Unresolved,
            };
            self.model.include_graph.directives.push(SourceIncludeDirective {
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
