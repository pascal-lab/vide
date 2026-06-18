use super::*;

impl SourcePreprocModel {
    pub(super) fn event_from_record(
        &self,
        source_order: usize,
        directive: &SourcePreprocEventRecord,
    ) -> Option<SourcePreprocEvent<'_>> {
        match directive.kind {
            MacroEventKind::Define => {
                let define = self.index.defines.get(directive.index)?;
                Some(SourcePreprocEvent::Define {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    define,
                })
            }
            MacroEventKind::Undef => {
                let undef = self.index.undefs.get(directive.index)?;
                Some(SourcePreprocEvent::Undef {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    undef,
                })
            }
            MacroEventKind::Include => {
                let include = self.index.includes.get(directive.index)?;
                Some(SourcePreprocEvent::Include {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    include,
                })
            }
            MacroEventKind::Conditional => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Conditional {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    conditional,
                })
            }
            MacroEventKind::Branch => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(SourcePreprocEvent::Branch {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    conditional,
                })
            }
            MacroEventKind::Usage => {
                let usage = self.index.usages.get(directive.index)?;
                Some(SourcePreprocEvent::Usage {
                    source_order,
                    event_id: directive.event_id,
                    index: directive.index,
                    usage,
                })
            }
        }
    }
}
