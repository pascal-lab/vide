use hir::{
    base_db::source_db::SourceDb,
    hir_def::macro_file::MacroExpansionDefinition,
    preproc::{MacroDefinition, MacroParamDefinition},
};
use vfs::FileId;

use super::expansion::macro_expansion_hover_text;
use crate::{
    db::root_db::RootDb,
    markup::{Markup, inline_code},
    render,
};

pub(super) fn render_macro_expansion_header(
    markup: &mut Markup,
    definition: &MacroExpansionDefinition,
) {
    markup.title(&macro_expansion_title(definition));
    match definition {
        MacroExpansionDefinition::Source(definition) => {
            markup.push_with_code_fence(&macro_signature(definition));
        }
        MacroExpansionDefinition::Builtin { name, .. } => {
            markup.push_with_code_fence(&format!("`{name}"));
        }
    }
}

fn macro_expansion_title(definition: &MacroExpansionDefinition) -> String {
    match definition {
        MacroExpansionDefinition::Source(definition) => macro_title(definition.name.as_str()),
        MacroExpansionDefinition::Builtin { name, .. } => macro_title(name.as_str()),
    }
}

fn macro_title(name: &str) -> String {
    format!("Macro {}", inline_code(name))
}

fn macro_signature(definition: &MacroDefinition) -> String {
    let mut signature = format!("`{}", definition.name);
    if let Some(params) = &definition.params {
        signature.push('(');
        for (index, param) in params.iter().enumerate() {
            if index > 0 {
                signature.push_str(", ");
            }
            signature.push_str(param.name.as_deref().unwrap_or("<unnamed>"));
        }
        signature.push(')');
    }
    signature
}

fn macro_definition_line(db: &RootDb, definition: &MacroDefinition) -> String {
    source_macro_definition_text(db, definition)
        .unwrap_or_else(|| fallback_macro_definition_line(definition))
}

fn source_macro_definition_text(db: &RootDb, definition: &MacroDefinition) -> Option<String> {
    let source = db.file_text(definition.file_id);
    let start = usize::from(definition.source_range.start());
    let end = usize::from(definition.source_range.end());
    let raw = source.get(start..end)?;
    raw.trim_start().starts_with("`define").then(|| macro_expansion_hover_text(raw))
}

fn fallback_macro_definition_line(definition: &MacroDefinition) -> String {
    let mut line = String::from("`define ");
    line.push_str(definition.name.as_str());
    if let Some(params) = &definition.params {
        line.push('(');
        for (index, param) in params.iter().enumerate() {
            if index > 0 {
                line.push_str(", ");
            }
            line.push_str(param.name.as_deref().unwrap_or("<unnamed>"));
        }
        line.push(')');
    }
    line
}

pub(super) fn macro_param_definition_markup(definition: &MacroParamDefinition) -> Markup {
    macro_param_definitions_markup(std::slice::from_ref(definition))
}

pub(super) fn macro_param_definitions_markup(definitions: &[MacroParamDefinition]) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        let definition = &definitions[0];
        markup.title(&format!("Macro parameter {}", inline_code(definition.name.as_str())));
        markup.metadata_line(&format!(
            "in macro {}",
            inline_code(definition.macro_definition.name.as_str())
        ));
        return markup;
    }

    markup.title("Macro parameters");
    markup.section("Candidates");
    for definition in definitions {
        if !markup.as_str().ends_with('\n') {
            markup.print("\n");
        }
        markup.print("- ");
        markup.print(&inline_code(definition.name.as_str()));
        markup.print(" of ");
        markup.print(&inline_code(definition.macro_definition.name.as_str()));
    }
    markup
}

pub(super) fn macro_definition_markup(
    db: &RootDb,
    anchor_file_id: FileId,
    definition: &MacroDefinition,
) -> Markup {
    macro_definitions_markup(db, anchor_file_id, std::slice::from_ref(definition))
}

pub(super) fn macro_definitions_markup(
    db: &RootDb,
    anchor_file_id: FileId,
    definitions: &[MacroDefinition],
) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        render_macro_definition_display(db, &mut markup, anchor_file_id, &definitions[0]);
        return markup;
    }

    markup.title("Macro definitions");
    markup.section("Candidates");
    for definition in definitions {
        if !markup.as_str().ends_with('\n') {
            markup.print("\n");
        }
        markup.print("- ");
        markup.print(&inline_code(definition.name.as_str()));
        if let Some(source) = macro_definition_source_fact(db, definition, anchor_file_id) {
            markup.print(" ");
            markup.print(&source);
        }
    }
    markup
}

fn render_macro_definition_display(
    db: &RootDb,
    markup: &mut Markup,
    anchor_file_id: FileId,
    definition: &MacroDefinition,
) {
    markup.title(&macro_title(definition.name.as_str()));
    markup.push_with_code_fence(&macro_definition_line(db, definition));
    let source = macro_definition_source_fact(db, definition, anchor_file_id)
        .unwrap_or_else(|| "unavailable".to_string());
    markup.metadata_line(&format!("from {source}"));
}

fn macro_definition_source_fact(
    db: &RootDb,
    definition: &MacroDefinition,
    anchor_file_id: FileId,
) -> Option<String> {
    macro_file_source_fact(db, definition.file_id, definition.source_range.start(), anchor_file_id)
}

pub(super) fn macro_expansion_source_fact(
    db: &RootDb,
    definition: &MacroExpansionDefinition,
    anchor_file_id: FileId,
) -> Option<String> {
    match definition {
        MacroExpansionDefinition::Source(definition) => {
            macro_definition_source_fact(db, definition, anchor_file_id)
        }
        MacroExpansionDefinition::Builtin { .. } => Some("builtin".to_string()),
    }
}

fn macro_file_source_fact(
    db: &RootDb,
    file_id: FileId,
    offset: utils::line_index::TextSize,
    anchor_file_id: FileId,
) -> Option<String> {
    render::source_line_link(db, file_id, offset, anchor_file_id)
}
