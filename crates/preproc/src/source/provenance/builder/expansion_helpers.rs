use super::*;

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn macro_reference_exists(
        &self,
        name: &str,
        name_range: SourceRange,
        site: &SourceMacroReferenceSite,
        resolution: &SourceMacroResolution,
    ) -> bool {
        self.tables.macro_references.iter().any(|reference| {
            reference.name.as_str() == name
                && reference.name_range == name_range
                && &reference.site == site
                && &reference.resolution == resolution
        })
    }

    pub(in crate::source::provenance::builder) fn direct_owned_emitted_tokens_by_call(
        &self,
    ) -> BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>> {
        let mut tokens_by_call = BTreeMap::<SourceMacroCallId, Vec<SourceEmittedTokenId>>::new();
        for (token, call) in &self.emitted_token_owners {
            tokens_by_call.entry(*call).or_default().push(*token);
        }
        tokens_by_call
    }

    pub(in crate::source::provenance::builder) fn expansion_definition_for_call(
        &self,
        call: SourceMacroCallId,
        direct_tokens_by_call: &BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>>,
    ) -> Option<SourceMacroExpansionDefinition> {
        if let Ok(definition) = self.definition_for_call(call) {
            return Some(SourceMacroExpansionDefinition::Source(definition));
        }

        let mut builtin_name = None;
        for token_id in direct_tokens_by_call.get(&call)? {
            let token = self.tables.emitted_tokens.get(*token_id)?;
            let provenance = self.tables.token_provenance.get(token.provenance)?;
            let SourceTokenProvenance::Builtin { name, .. } = provenance else {
                continue;
            };
            match &builtin_name {
                Some(existing) if existing != name => return None,
                Some(_) => {}
                None => builtin_name = Some(name.clone()),
            }
        }
        builtin_name.map(|name| SourceMacroExpansionDefinition::Builtin { name })
    }

    pub(in crate::source::provenance::builder) fn child_calls_by_parent(
        &mut self,
    ) -> BTreeMap<SourceMacroCallId, Vec<SourceMacroCallId>> {
        let call_ids = self.tables.macro_calls.iter().map(|call| call.id).collect::<Vec<_>>();
        let mut children = BTreeMap::<SourceMacroCallId, Vec<SourceMacroCallId>>::new();
        for child in &call_ids {
            let Some(child_call) = self.tables.macro_calls.get(*child) else {
                self.expansions_partial = true;
                continue;
            };
            let Some(parent_expansion_identity) = child_call.parent_expansion_identity else {
                continue;
            };
            match self.call_ids_by_expansion_identity.get(&parent_expansion_identity).copied() {
                Some(parent) if parent != *child => {
                    children.entry(parent).or_default().push(*child);
                }
                Some(_) | None => {
                    self.expansions_partial = true;
                }
            }
        }
        for child_calls in children.values_mut() {
            child_calls.sort_by_key(|call| call.raw());
            child_calls.dedup();
        }
        children
    }

    pub(in crate::source::provenance::builder) fn recursive_emitted_tokens_for_call(
        &mut self,
        call: SourceMacroCallId,
        direct_tokens_by_call: &BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>>,
        child_calls_by_parent: &BTreeMap<SourceMacroCallId, Vec<SourceMacroCallId>>,
        recursive_tokens_by_call: &mut BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>>,
        visiting: &mut Vec<SourceMacroCallId>,
    ) -> Vec<SourceEmittedTokenId> {
        if let Some(tokens) = recursive_tokens_by_call.get(&call) {
            return tokens.clone();
        }
        if visiting.contains(&call) {
            self.expansions_partial = true;
            return Vec::new();
        }

        visiting.push(call);
        let mut tokens = direct_tokens_by_call.get(&call).cloned().unwrap_or_default();
        if let Some(children) = child_calls_by_parent.get(&call) {
            for child in children {
                tokens.extend(self.recursive_emitted_tokens_for_call(
                    *child,
                    direct_tokens_by_call,
                    child_calls_by_parent,
                    recursive_tokens_by_call,
                    visiting,
                ));
            }
        }
        visiting.pop();
        tokens.sort_by_key(|token| token.raw());
        tokens.dedup();
        recursive_tokens_by_call.insert(call, tokens.clone());
        tokens
    }

    pub(in crate::source::provenance::builder) fn mark_call_unavailable(
        &mut self,
        call: SourceMacroCallId,
        reason: SourcePreprocUnavailable,
    ) {
        self.expansions_partial = true;
        if let Some(call) = self.tables.macro_calls.get_mut(call) {
            call.expansion = None;
            call.status = SourceMacroCallStatus::ExpansionUnavailable(reason);
        }
    }

    pub(in crate::source::provenance::builder) fn record_emitted_token_owner(
        &mut self,
        token: SourceEmittedTokenId,
        call: SourceMacroCallId,
    ) {
        self.emitted_token_owners.insert(token, call);
    }

    pub(in crate::source::provenance::builder) fn source_is_predefine(
        &self,
        source: PreprocSourceId,
    ) -> bool {
        self.index.sources.iter().any(|candidate| {
            candidate.id == source && candidate.origin == PreprocSourceOrigin::Predefine
        })
    }

    pub(in crate::source::provenance::builder) fn unavailable_token_provenance(
        &mut self,
        reason: SourcePreprocUnavailable,
    ) -> SourceTokenProvenance {
        self.token_provenance_partial = true;
        SourceTokenProvenance::Unavailable(reason)
    }
}
