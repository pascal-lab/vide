use smol_str::SmolStr;

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroReferenceResolution<'a> {
    pub site: SourceMacroReferenceSite,
    pub name: SmolStr,
    pub range: SourceRange,
    pub definition: SourceMacroBinding<'a>,
    pub definition_provenance: SourcePreprocProvenance,
    pub definition_include_chain: Vec<SourceIncludeChainEntry>,
}

impl SourcePreprocModel {
    pub fn definition_for_usage(
        &self,
        usage_index: usize,
    ) -> Result<Option<SourceMacroUsageResolution<'_>>, SourcePreprocError> {
        let Some(usage) = self.index.usages.get(usage_index) else {
            return Ok(None);
        };
        let Some(reference) = self.reference_for_usage(usage_index) else {
            return Ok(None);
        };
        let SourceMacroResolution::Resolved { definition, include_chain, .. } =
            &reference.resolution
        else {
            return unavailable_reference_result(&reference.resolution);
        };
        let Some(definition) = self.binding_for_definition_id(*definition) else {
            return Ok(None);
        };
        let definition_provenance = self
            .provenance(SourcePreprocEntity::Define(definition.define_index))
            .ok_or(SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() })?;
        Ok(Some(SourceMacroUsageResolution {
            usage_index,
            usage,
            definition,
            definition_provenance,
            definition_include_chain: include_chain.clone(),
        }))
    }

    pub fn definition_for_conditional_token(
        &self,
        conditional_index: usize,
        token_index: usize,
    ) -> Option<SourceMacroBinding<'_>> {
        let reference = self.reference_for_conditional_token(conditional_index, token_index)?;
        let SourceMacroResolution::Resolved { definition, .. } = reference.resolution else {
            return None;
        };
        self.binding_for_definition_id(definition)
    }

    pub fn resolved_macro_references(
        &self,
    ) -> Result<Vec<SourceMacroReferenceResolution<'_>>, SourcePreprocError> {
        let mut references = Vec::new();

        for reference in self.tables.macro_references.iter() {
            let SourceMacroResolution::Resolved { definition, include_chain, .. } =
                &reference.resolution
            else {
                if let Some(error) = source_error_for_unavailable_resolution(&reference.resolution)
                {
                    return Err(error);
                }
                continue;
            };
            let Some(definition) = self.binding_for_definition_id(*definition) else {
                continue;
            };
            let definition_provenance = self
                .provenance(SourcePreprocEntity::Define(definition.define_index))
                .ok_or(SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() })?;
            references.push(SourceMacroReferenceResolution {
                site: reference.site,
                name: reference.name.clone(),
                range: reference.name_range,
                definition,
                definition_provenance,
                definition_include_chain: include_chain.clone(),
            });
        }

        Ok(references)
    }

    fn reference_for_usage(&self, usage_index: usize) -> Option<&SourceMacroReference> {
        self.tables.macro_references.iter().find(|reference| {
            matches!(reference.site, SourceMacroReferenceSite::Usage {
                usage_index: site_usage_index,
            } if site_usage_index == usage_index)
        })
    }

    fn reference_for_conditional_token(
        &self,
        conditional_index: usize,
        token_index: usize,
    ) -> Option<&SourceMacroReference> {
        self.tables.macro_references.iter().find(|reference| {
            matches!(
                reference.site,
                SourceMacroReferenceSite::ConditionalToken {
                    conditional_index: site_conditional_index,
                    token_index: site_token_index,
                } | SourceMacroReferenceSite::IncludeGuardIfNDef {
                    conditional_index: site_conditional_index,
                    token_index: site_token_index,
                } if site_conditional_index == conditional_index && site_token_index == token_index
            )
        })
    }
}

fn unavailable_reference_result<'a>(
    resolution: &SourceMacroResolution,
) -> Result<Option<SourceMacroUsageResolution<'a>>, SourcePreprocError> {
    if let Some(error) = source_error_for_unavailable_resolution(resolution) {
        return Err(error);
    }

    Ok(None)
}

fn source_error_for_unavailable_resolution(
    resolution: &SourceMacroResolution,
) -> Option<SourcePreprocError> {
    let SourceMacroResolution::Unavailable(unavailable) = resolution else {
        return None;
    };

    match unavailable {
        SourcePreprocUnavailable::MissingIncludedSource { include_event_id, source } => {
            Some(SourcePreprocError::MissingIncludedSource {
                include_event_id: include_event_id.raw(),
                source: source.raw(),
            })
        }
        SourcePreprocUnavailable::MissingIncludeEvent { include_event_id } => {
            Some(SourcePreprocError::MissingIncludeEvent {
                include_event_id: include_event_id.raw(),
            })
        }
        SourcePreprocUnavailable::IncludeEdgeNotInclude { include_event_id } => {
            Some(SourcePreprocError::IncludeEdgeNotInclude {
                include_event_id: include_event_id.raw(),
            })
        }
        SourcePreprocUnavailable::IncludeChainUnavailable { source }
        | SourcePreprocUnavailable::DetachedSource { source } => {
            Some(SourcePreprocError::MissingIncludeEdge { source: source.raw() })
        }
        SourcePreprocUnavailable::MissingDefinitionName { event_id }
        | SourcePreprocUnavailable::MissingDefinitionNameRange { event_id }
        | SourcePreprocUnavailable::MissingReferenceName { event_id }
        | SourcePreprocUnavailable::MissingReferenceNameRange { event_id } => {
            Some(SourcePreprocError::MissingEvent { event_id: event_id.raw() })
        }
        SourcePreprocUnavailable::MacroCallAuthorityUnavailable
        | SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable
        | SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable
        | SourcePreprocUnavailable::ExpansionAuthorityUnavailable => None,
    }
}
