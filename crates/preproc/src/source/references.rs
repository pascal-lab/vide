use smol_str::SmolStr;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMacroReferenceSite {
    Usage { usage_index: usize },
    ConditionalToken { conditional_index: usize, token_index: usize },
}

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
    ) -> Result<Option<SourceMacroResolution<'_>>, SourcePreprocError> {
        let Some(usage) = self.index.usages.get(usage_index) else {
            return Ok(None);
        };
        let Some(name) = usage.name.as_ref() else {
            return Ok(None);
        };
        let Some(environment) =
            self.macro_environment_before(SourcePreprocEntity::Usage(usage_index))
        else {
            return Ok(None);
        };
        let Some(define_index) = environment.define_index(name.as_str()) else {
            return Ok(None);
        };
        let Some(define) = self.index.defines.get(define_index) else {
            return Ok(None);
        };
        let definition = SourceMacroBinding {
            name: name.clone(),
            event_id: define.event_id,
            define_index,
            define,
        };
        let definition_provenance = self
            .provenance(SourcePreprocEntity::Define(define_index))
            .ok_or(SourcePreprocError::MissingEvent { event_id: define.event_id.raw() })?;
        let definition_include_chain = self.include_chain_for_source(define.range.source)?;
        Ok(Some(SourceMacroResolution {
            usage_index,
            usage,
            definition,
            definition_provenance,
            definition_include_chain,
        }))
    }

    pub fn definition_for_conditional_token(
        &self,
        conditional_index: usize,
        token_index: usize,
    ) -> Option<SourceMacroBinding<'_>> {
        let conditional = self.index.conditionals.get(conditional_index)?;
        let token = conditional.expr.get(token_index)?;
        token.range?;
        let environment =
            self.macro_environment_before(SourcePreprocEntity::Conditional(conditional_index))?;
        if let Some(define_index) = environment.define_index(token.value.as_str()) {
            return self.binding_for_define_index(token.value.clone(), define_index);
        }

        self.include_guard_definition_after_ifndef(conditional_index, token.value.as_str())
    }

    pub fn resolved_macro_references(
        &self,
    ) -> Result<Vec<SourceMacroReferenceResolution<'_>>, SourcePreprocError> {
        let mut references = Vec::new();

        for (usage_index, usage) in self.index.usages.iter().enumerate() {
            let Some(resolution) = self.definition_for_usage(usage_index)? else {
                continue;
            };
            let Some(name) = usage.name.clone() else {
                continue;
            };
            references.push(SourceMacroReferenceResolution {
                site: SourceMacroReferenceSite::Usage { usage_index },
                name,
                range: usage.range,
                definition: resolution.definition,
                definition_provenance: resolution.definition_provenance,
                definition_include_chain: resolution.definition_include_chain,
            });
        }

        for (conditional_index, conditional) in self.index.conditionals.iter().enumerate() {
            for (token_index, token) in conditional.expr.iter().enumerate() {
                let Some(range) = token.range else {
                    continue;
                };
                let Some(definition) =
                    self.definition_for_conditional_token(conditional_index, token_index)
                else {
                    continue;
                };
                let definition_provenance =
                    self.provenance(SourcePreprocEntity::Define(definition.define_index)).ok_or(
                        SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() },
                    )?;
                let definition_include_chain =
                    self.include_chain_for_source(definition.define.range.source)?;
                references.push(SourceMacroReferenceResolution {
                    site: SourceMacroReferenceSite::ConditionalToken {
                        conditional_index,
                        token_index,
                    },
                    name: token.value.clone(),
                    range,
                    definition,
                    definition_provenance,
                    definition_include_chain,
                });
            }
        }

        Ok(references)
    }

    fn include_guard_definition_after_ifndef(
        &self,
        conditional_index: usize,
        name: &str,
    ) -> Option<SourceMacroBinding<'_>> {
        let conditional = self.index.conditionals.get(conditional_index)?;
        if conditional.kind != MacroConditionalKind::IfNDef {
            return None;
        }

        // Include guards are intentional forward references: at `ifndef GUARD`,
        // normal macro visibility says GUARD is not defined yet. For navigation
        // we model only the canonical same-source guard shape by binding that
        // token to the following same-name `define` before any branch boundary.
        // This is collected into the resolved-reference model, not used as a
        // path, text, or IDE-layer fallback.
        let source = conditional.range.source;
        let (conditional_order, _) =
            self.event_record_for_entity(SourcePreprocEntity::Conditional(conditional_index))?;
        for directive in self.index.event_records.iter().skip(conditional_order + 1) {
            if directive.range.source != source {
                continue;
            }
            match directive.kind {
                MacroEventKind::Define => {
                    let define = self.index.defines.get(directive.index)?;
                    if define.name.as_deref() == Some(name) {
                        return self.binding_for_define_index(SmolStr::new(name), directive.index);
                    }
                }
                MacroEventKind::Branch => break,
                MacroEventKind::Undef
                | MacroEventKind::Include
                | MacroEventKind::Conditional
                | MacroEventKind::Usage => {}
            }
        }

        None
    }
}
