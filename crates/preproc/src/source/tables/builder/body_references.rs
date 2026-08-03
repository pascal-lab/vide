use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn record_macro_body_references_for_calls(&mut self) {
        let calls = self.model.macro_calls.iter().cloned().collect::<Vec<_>>();
        for call in calls {
            let Some(reference) = self.model.macro_references.get(call.reference).cloned() else {
                continue;
            };
            let SourceMacroResolution::Resolved { definition, .. } = reference.resolution else {
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

    pub(in crate::source::tables::builder) fn macro_reference_exists(
        &self,
        name: &str,
        name_range: SourceRange,
        site: &SourceMacroReferenceSite,
        resolution: &SourceMacroResolution,
    ) -> bool {
        self.model.macro_references.iter().any(|reference| {
            reference.name.as_str() == name
                && reference.name_range == name_range
                && &reference.site == site
                && &reference.resolution == resolution
        })
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
