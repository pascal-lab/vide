use std::collections::BTreeMap;

use smol_str::SmolStr;
use syntax::SyntaxTreeOptions;
use utils::line_index::{TextRange, TextSize};

use crate::index::{
    MacroConditional, MacroDefine, MacroDirective, MacroDirectiveKind, MacroInclude, MacroUndef,
    MacroUsage, PreprocFileIndex, preproc_file_index_from_text,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocModel {
    index: PreprocFileIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroEnvironment {
    definitions: BTreeMap<SmolStr, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroBinding<'a> {
    pub name: SmolStr,
    pub define_index: usize,
    pub define: &'a MacroDefine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocEntity {
    Define(usize),
    Undef(usize),
    Usage(usize),
    Include(usize),
    Conditional(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocProvenance {
    pub entity: PreprocEntity,
    pub name: Option<SmolStr>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocEvent<'a> {
    Define { source_order: usize, index: usize, define: &'a MacroDefine },
    Undef { source_order: usize, index: usize, undef: &'a MacroUndef },
    Include { source_order: usize, index: usize, include: &'a MacroInclude },
    Conditional { source_order: usize, index: usize, conditional: &'a MacroConditional },
    Branch { source_order: usize, index: usize, conditional: &'a MacroConditional },
    Usage { source_order: usize, index: usize, usage: &'a MacroUsage },
}

impl PreprocModel {
    pub fn new(index: PreprocFileIndex) -> Self {
        Self { index }
    }

    pub fn from_text(text: &str, options: &SyntaxTreeOptions) -> Self {
        Self::new(preproc_file_index_from_text(text, options))
    }

    pub fn index(&self) -> &PreprocFileIndex {
        &self.index
    }

    pub fn into_index(self) -> PreprocFileIndex {
        self.index
    }

    pub fn defines(&self) -> &[MacroDefine] {
        &self.index.defines
    }

    pub fn undefs(&self) -> &[MacroUndef] {
        &self.index.undefs
    }

    pub fn usages(&self) -> &[MacroUsage] {
        &self.index.usages
    }

    pub fn includes(&self) -> &[MacroInclude] {
        &self.index.includes
    }

    pub fn conditionals(&self) -> &[MacroConditional] {
        &self.index.conditionals
    }

    pub fn inactive_ranges(&self) -> &[TextRange] {
        &self.index.inactive_ranges
    }

    pub fn events(&self) -> impl Iterator<Item = PreprocEvent<'_>> + '_ {
        self.index.directives.iter().enumerate().filter_map(|(source_order, directive)| {
            self.event_from_directive(source_order, directive)
        })
    }

    pub fn macro_environment_at(&self, offset: TextSize) -> MacroEnvironment {
        let mut environment = MacroEnvironment::default();
        for directive in &self.index.directives {
            if directive_applies_at_offset(directive, offset) {
                self.apply_macro_state(directive, &mut environment);
            }
        }
        environment
    }

    pub fn visible_macros_at(&self, offset: TextSize) -> Vec<MacroBinding<'_>> {
        let environment = self.macro_environment_at(offset);
        self.bindings_for_environment(&environment)
    }

    pub fn definition_for_usage(&self, usage_index: usize) -> Option<MacroBinding<'_>> {
        let usage = self.index.usages.get(usage_index)?;
        let name = usage.name.as_ref()?;
        let environment = self.macro_environment_before(PreprocEntity::Usage(usage_index))?;
        let define_index = environment.define_index(name.as_str())?;
        let define = self.index.defines.get(define_index)?;
        Some(MacroBinding { name: name.clone(), define_index, define })
    }

    pub fn provenance(&self, entity: PreprocEntity) -> Option<PreprocProvenance> {
        let (name, range) = match entity {
            PreprocEntity::Define(index) => {
                let define = self.index.defines.get(index)?;
                (define.name.clone(), define.range)
            }
            PreprocEntity::Undef(index) => {
                let undef = self.index.undefs.get(index)?;
                (undef.name.clone(), undef.range)
            }
            PreprocEntity::Usage(index) => {
                let usage = self.index.usages.get(index)?;
                (usage.name.clone(), usage.range)
            }
            PreprocEntity::Include(index) => {
                let include = self.index.includes.get(index)?;
                (None, include.range)
            }
            PreprocEntity::Conditional(index) => {
                let conditional = self.index.conditionals.get(index)?;
                (None, conditional.range)
            }
        };
        Some(PreprocProvenance { entity, name, range })
    }

    pub fn source_range(&self, entity: PreprocEntity) -> Option<TextRange> {
        self.provenance(entity).and_then(|provenance| provenance.range)
    }

    pub fn define(&self, index: usize) -> Option<&MacroDefine> {
        self.index.defines.get(index)
    }

    pub fn undef(&self, index: usize) -> Option<&MacroUndef> {
        self.index.undefs.get(index)
    }

    pub fn usage(&self, index: usize) -> Option<&MacroUsage> {
        self.index.usages.get(index)
    }

    pub fn include(&self, index: usize) -> Option<&MacroInclude> {
        self.index.includes.get(index)
    }

    pub fn conditional(&self, index: usize) -> Option<&MacroConditional> {
        self.index.conditionals.get(index)
    }

    fn macro_environment_before(&self, entity: PreprocEntity) -> Option<MacroEnvironment> {
        let mut environment = MacroEnvironment::default();
        for directive in &self.index.directives {
            if directive_matches_entity(directive, entity) {
                return Some(environment);
            }
            self.apply_macro_state(directive, &mut environment);
        }
        None
    }

    fn bindings_for_environment(&self, environment: &MacroEnvironment) -> Vec<MacroBinding<'_>> {
        environment
            .definitions
            .iter()
            .filter_map(|(name, define_index)| {
                let define = self.index.defines.get(*define_index)?;
                Some(MacroBinding { name: name.clone(), define_index: *define_index, define })
            })
            .collect()
    }

    fn apply_macro_state(&self, directive: &MacroDirective, environment: &mut MacroEnvironment) {
        match directive.kind {
            MacroDirectiveKind::Define => {
                if let Some(define) = self.index.defines.get(directive.index)
                    && let Some(name) = define.name.as_ref()
                {
                    environment.definitions.insert(name.clone(), directive.index);
                }
            }
            MacroDirectiveKind::Undef => {
                if let Some(undef) = self.index.undefs.get(directive.index)
                    && let Some(name) = undef.name.as_ref()
                {
                    environment.definitions.remove(name.as_str());
                }
            }
            MacroDirectiveKind::Include
            | MacroDirectiveKind::Conditional
            | MacroDirectiveKind::Branch
            | MacroDirectiveKind::Usage => {}
        }
    }

    fn event_from_directive(
        &self,
        source_order: usize,
        directive: &MacroDirective,
    ) -> Option<PreprocEvent<'_>> {
        match directive.kind {
            MacroDirectiveKind::Define => {
                let define = self.index.defines.get(directive.index)?;
                Some(PreprocEvent::Define { source_order, index: directive.index, define })
            }
            MacroDirectiveKind::Undef => {
                let undef = self.index.undefs.get(directive.index)?;
                Some(PreprocEvent::Undef { source_order, index: directive.index, undef })
            }
            MacroDirectiveKind::Include => {
                let include = self.index.includes.get(directive.index)?;
                Some(PreprocEvent::Include { source_order, index: directive.index, include })
            }
            MacroDirectiveKind::Conditional => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(PreprocEvent::Conditional {
                    source_order,
                    index: directive.index,
                    conditional,
                })
            }
            MacroDirectiveKind::Branch => {
                let conditional = self.index.conditionals.get(directive.index)?;
                Some(PreprocEvent::Branch { source_order, index: directive.index, conditional })
            }
            MacroDirectiveKind::Usage => {
                let usage = self.index.usages.get(directive.index)?;
                Some(PreprocEvent::Usage { source_order, index: directive.index, usage })
            }
        }
    }
}

impl MacroEnvironment {
    pub fn define_index(&self, name: &str) -> Option<usize> {
        self.definitions.get(name).copied()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &SmolStr> {
        self.definitions.keys()
    }

    pub fn definitions(&self) -> &BTreeMap<SmolStr, usize> {
        &self.definitions
    }
}

impl PreprocEvent<'_> {
    pub fn source_order(&self) -> usize {
        match self {
            PreprocEvent::Define { source_order, .. }
            | PreprocEvent::Undef { source_order, .. }
            | PreprocEvent::Include { source_order, .. }
            | PreprocEvent::Conditional { source_order, .. }
            | PreprocEvent::Branch { source_order, .. }
            | PreprocEvent::Usage { source_order, .. } => *source_order,
        }
    }

    pub fn kind(&self) -> MacroDirectiveKind {
        match self {
            PreprocEvent::Define { .. } => MacroDirectiveKind::Define,
            PreprocEvent::Undef { .. } => MacroDirectiveKind::Undef,
            PreprocEvent::Include { .. } => MacroDirectiveKind::Include,
            PreprocEvent::Conditional { .. } => MacroDirectiveKind::Conditional,
            PreprocEvent::Branch { .. } => MacroDirectiveKind::Branch,
            PreprocEvent::Usage { .. } => MacroDirectiveKind::Usage,
        }
    }

    pub fn entity(&self) -> PreprocEntity {
        match self {
            PreprocEvent::Define { index, .. } => PreprocEntity::Define(*index),
            PreprocEvent::Undef { index, .. } => PreprocEntity::Undef(*index),
            PreprocEvent::Include { index, .. } => PreprocEntity::Include(*index),
            PreprocEvent::Conditional { index, .. } | PreprocEvent::Branch { index, .. } => {
                PreprocEntity::Conditional(*index)
            }
            PreprocEvent::Usage { index, .. } => PreprocEntity::Usage(*index),
        }
    }

    pub fn range(&self) -> Option<TextRange> {
        match self {
            PreprocEvent::Define { define, .. } => define.range,
            PreprocEvent::Undef { undef, .. } => undef.range,
            PreprocEvent::Include { include, .. } => include.range,
            PreprocEvent::Conditional { conditional, .. }
            | PreprocEvent::Branch { conditional, .. } => conditional.range,
            PreprocEvent::Usage { usage, .. } => usage.range,
        }
    }
}

fn directive_applies_at_offset(directive: &MacroDirective, offset: TextSize) -> bool {
    directive.range.is_none_or(|range| range.end() <= offset)
}

fn directive_matches_entity(directive: &MacroDirective, entity: PreprocEntity) -> bool {
    match (directive.kind, entity) {
        (MacroDirectiveKind::Define, PreprocEntity::Define(index))
        | (MacroDirectiveKind::Undef, PreprocEntity::Undef(index))
        | (MacroDirectiveKind::Usage, PreprocEntity::Usage(index))
        | (MacroDirectiveKind::Include, PreprocEntity::Include(index)) => directive.index == index,
        (
            MacroDirectiveKind::Conditional | MacroDirectiveKind::Branch,
            PreprocEntity::Conditional(index),
        ) => directive.index == index,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;

    use super::*;
    use crate::index::{MacroConditionalKind, MacroIncludeTarget};

    fn model(text: &str) -> PreprocModel {
        PreprocModel::from_text(text, &SyntaxTreeOptions::without_include_expansion())
    }

    fn model_with_predefines(text: &str, predefines: Vec<String>) -> PreprocModel {
        PreprocModel::from_text(
            text,
            &SyntaxTreeOptions { predefines, ..SyntaxTreeOptions::without_include_expansion() },
        )
    }

    fn offset_after(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
    }

    fn text_at_range(text: &str, range: TextRange) -> &str {
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn preproc_model_reports_define_visible_after_directive() {
        let text = r#"`define WIDTH 8
module top;
endmodule
"#;
        let model = model(text);
        let environment = model.macro_environment_at(offset_after(text, "`define WIDTH 8\n"));

        assert_eq!(environment.define_index("WIDTH"), Some(0));
        assert_eq!(
            environment.names().map(|name| name.as_str()).collect::<Vec<_>>(),
            vec!["WIDTH"]
        );

        let visible = model.visible_macros_at(offset_after(text, "`define WIDTH 8\n"));
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name.as_str(), "WIDTH");
        assert_eq!(visible[0].define_index, 0);

        let provenance = model.provenance(PreprocEntity::Define(0)).unwrap();
        assert_eq!(provenance.name.as_deref(), Some("WIDTH"));
        assert_eq!(text_at_range(text, provenance.range.unwrap()).trim(), "WIDTH");
    }

    #[test]
    fn preproc_model_removes_macro_after_undef() {
        let text = r#"`define WIDTH 8
`undef WIDTH
module top;
endmodule
"#;
        let model = model(text);
        let environment = model.macro_environment_at(offset_after(text, "`undef WIDTH\n"));

        assert!(!environment.contains("WIDTH"));
        assert_eq!(environment.define_index("WIDTH"), None);

        let provenance = model.provenance(PreprocEntity::Undef(0)).unwrap();
        assert_eq!(provenance.name.as_deref(), Some("WIDTH"));
        assert_eq!(text_at_range(text, provenance.range.unwrap()), "WIDTH");
    }

    #[test]
    fn preproc_model_uses_latest_define_for_same_macro_name() {
        let text = r#"`define WIDTH 8
`define WIDTH 16
module top;
endmodule
"#;
        let model = model(text);
        let environment = model.macro_environment_at(offset_after(text, "`define WIDTH 16\n"));

        assert_eq!(environment.define_index("WIDTH"), Some(1));
        let binding =
            model.visible_macros_at(offset_after(text, "`define WIDTH 16\n")).pop().unwrap();
        assert_eq!(binding.define_index, 1);
        assert_eq!(binding.define.body[0].value.as_str(), "16");
    }

    #[test]
    fn preproc_model_resolves_usage_to_nearest_effective_define() {
        let text = r#"`define WIDTH 8
logic [`WIDTH-1:0] a;
`define WIDTH 16
logic [`WIDTH-1:0] b;
"#;
        let model = model(text);

        assert_eq!(model.usages().len(), 2);

        let first_binding = model.definition_for_usage(0).unwrap();
        assert_eq!(first_binding.name.as_str(), "WIDTH");
        assert_eq!(first_binding.define_index, 0);
        assert_eq!(first_binding.define.body[0].value.as_str(), "8");

        let second_binding = model.definition_for_usage(1).unwrap();
        assert_eq!(second_binding.name.as_str(), "WIDTH");
        assert_eq!(second_binding.define_index, 1);
        assert_eq!(second_binding.define.body[0].value.as_str(), "16");

        let provenance = model.provenance(PreprocEntity::Usage(1)).unwrap();
        assert_eq!(provenance.name.as_deref(), Some("WIDTH"));
        assert_eq!(text_at_range(text, provenance.range.unwrap()), "`WIDTH");
    }

    #[test]
    fn preproc_model_preserves_conditional_branch_ranges() {
        let text = r#"`ifdef USE_A
logic active;
`else
logic inactive;
`endif
"#;
        let model = model_with_predefines(text, vec!["USE_A=1".to_owned()]);

        assert_eq!(
            model.conditionals().iter().map(|conditional| conditional.kind).collect::<Vec<_>>(),
            vec![
                MacroConditionalKind::IfDef,
                MacroConditionalKind::Else,
                MacroConditionalKind::EndIf,
            ]
        );
        assert_eq!(
            model.events().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                MacroDirectiveKind::Conditional,
                MacroDirectiveKind::Branch,
                MacroDirectiveKind::Branch,
            ]
        );

        let else_range = model.source_range(PreprocEntity::Conditional(1)).unwrap();
        assert_eq!(text_at_range(text, else_range), "logic inactive");
        let inactive_range = model.inactive_ranges()[0];
        assert_eq!(text_at_range(text, inactive_range), "logic inactive;");
    }

    #[test]
    fn preproc_model_exposes_include_targets() {
        let text = r#"`include "defs.svh"
module top;
endmodule
"#;
        let model = model(text);

        assert_eq!(
            model.includes()[0].target,
            MacroIncludeTarget::Literal {
                path: SmolStr::new("defs.svh"),
                raw: SmolStr::new("\"defs.svh\"")
            }
        );
        assert_eq!(
            model.events().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![MacroDirectiveKind::Include]
        );

        let include_range = model.source_range(PreprocEntity::Include(0)).unwrap();
        assert_eq!(text_at_range(text, include_range), "\"defs.svh\"");
    }
}
