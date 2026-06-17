use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::provenance::builder) fn build_macro_expansion_graph(&mut self) {
        if self.model.macro_calls.is_empty() {
            return;
        }

        let direct_tokens_by_call = self.direct_owned_emitted_tokens_by_call();
        let child_calls_by_parent = self.child_calls_by_parent();
        let call_ids = self.model.macro_calls.iter().map(|call| call.id).collect::<Vec<_>>();
        let mut expansion_tokens_by_call = BTreeMap::new();
        let mut recursive_tokens_by_call = BTreeMap::new();
        for call in &call_ids {
            let mut visiting = Vec::new();
            let tokens = self.recursive_emitted_tokens_for_call(
                *call,
                &direct_tokens_by_call,
                &child_calls_by_parent,
                &mut recursive_tokens_by_call,
                &mut visiting,
            );
            expansion_tokens_by_call.insert(*call, tokens);
        }

        for call in call_ids {
            let tokens = expansion_tokens_by_call.remove(&call).unwrap_or_default();
            let Some(expansion_identity) =
                self.model.macro_calls.get(call).and_then(|call| call.expansion_identity)
            else {
                self.mark_call_unavailable(
                    call,
                    SourcePreprocUnavailable::MissingMacroExpansion { call },
                );
                continue;
            };
            let Some(emitted_token_range) = tokens.contiguous_emitted_range(
                SourceEmittedTokenId::new(self.model.emitted_tokens.len()),
            ) else {
                self.mark_call_unavailable(
                    call,
                    SourcePreprocUnavailable::MissingMacroExpansion { call },
                );
                continue;
            };
            let Some(definition) = self.expansion_definition_for_call(call, &direct_tokens_by_call)
            else {
                self.mark_call_unavailable(
                    call,
                    SourcePreprocUnavailable::MissingMacroExpansion { call },
                );
                continue;
            };

            let expansion = SourceMacroExpansionId::new(self.model.macro_expansions.len());
            self.model.macro_expansions.push(SourceMacroExpansion {
                id: expansion,
                identity: Some(expansion_identity),
                call,
                definition,
                emitted_token_range,
                child_calls: child_calls_by_parent.get(&call).cloned().unwrap_or_default(),
                status: SourceMacroExpansionStatus::Complete,
            });
            if let Some(call) = self.model.macro_calls.get_mut(call) {
                call.expansion = Some(expansion);
                call.status = SourceMacroCallStatus::ExpansionAvailable;
            }
        }
    }

    pub(in crate::source::provenance::builder) fn record_macro_body_references_for_calls(
        &mut self,
    ) {
        let calls = self.model.macro_calls.iter().cloned().collect::<Vec<_>>();
        for call in calls {
            let SourceMacroResolution::Resolved { definition, .. } = call.callee else {
                continue;
            };
            let Some(definition) = self.model.macro_definitions.get(definition).cloned() else {
                continue;
            };
            let call_position = SourcePosition {
                source: call.call_range.source,
                offset: call.call_range.range.start(),
            };
            for (token_index, token) in definition.body_tokens.iter().enumerate() {
                let Some(name) = token.macro_reference_name() else {
                    continue;
                };
                let Some(name_range) = token.range else {
                    self.record_missing_reference_name_range(definition.event_id);
                    continue;
                };
                let resolution =
                    self.resolve_visible_reference_at_position(name.as_str(), call_position);
                let site = SourceMacroReferenceSite::MacroBodyToken { call: call.id, token_index };
                if self.macro_reference_exists(name.as_str(), name_range, &site, &resolution) {
                    continue;
                }
                self.push_reference(
                    definition.event_id,
                    site,
                    name,
                    name_range,
                    definition.directive_range,
                    resolution,
                );
            }
        }
    }
}

trait SourceMacroTokenExt {
    fn macro_reference_name(&self) -> Option<SmolStr>;
}

impl SourceMacroTokenExt for SourceMacroToken {
    fn macro_reference_name(&self) -> Option<SmolStr> {
        if !self.raw.starts_with('`') {
            return None;
        }
        let name = self.value.strip_prefix('`').unwrap_or(self.value.as_str());
        (!name.is_empty()).then(|| SmolStr::new(name))
    }
}

trait SourceEmittedTokenIdSliceExt {
    fn contiguous_emitted_range(
        &self,
        empty_start: SourceEmittedTokenId,
    ) -> Option<SourceEmittedTokenRange>;
}

impl SourceEmittedTokenIdSliceExt for [SourceEmittedTokenId] {
    fn contiguous_emitted_range(
        &self,
        empty_start: SourceEmittedTokenId,
    ) -> Option<SourceEmittedTokenRange> {
        let Some(first) = self.first().copied() else {
            return Some(SourceEmittedTokenRange { start: empty_start, len: 0 });
        };
        let last = *self.last()?;
        let len = last.raw().checked_sub(first.raw())? + 1;
        (len == self.len()).then_some(SourceEmittedTokenRange { start: first, len })
    }
}
