use base_db::source_db::SourceDb;
use preproc_expand::{
    macro_file::MacroExpansionDefinition,
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

fn macro_definition_line(db: &RootDb, definition: &MacroDefinition) -> Option<String> {
    let source = db.file_text(definition.file_id);
    let start = usize::from(definition.source_range.start());
    let end = usize::from(definition.source_range.end());
    let Some(raw) = source.get(start..end) else {
        tracing::error!(
            ?definition.source_range,
            file_id = ?definition.file_id,
            "macro definition source range is outside the source file"
        );
        return None;
    };
    if raw.trim_start().starts_with("`define") {
        return Some(macro_expansion_hover_text(raw));
    }
    if db.file_kind(definition.file_id).is_project_manifest() {
        if definition.params.is_some() {
            tracing::error!(
                macro_name = %definition.name,
                "vide.toml macro definition unexpectedly has parameters"
            );
            return None;
        }
        let line = manifest_macro_definition_line(raw, definition);
        if line.is_none() {
            tracing::error!(
                macro_name = %definition.name,
                "vide.toml macro definition value cannot be rendered"
            );
        }
        return line;
    }
    tracing::error!(
        ?definition.source_range,
        file_id = ?definition.file_id,
        macro_name = %definition.name,
        "macro definition source is neither SystemVerilog nor vide.toml"
    );
    None
}

fn manifest_macro_definition_line(raw: &str, definition: &MacroDefinition) -> Option<String> {
    let document = format!("value = {}", raw.trim());
    let parsed = toml::from_str::<toml::Value>(&document).ok()?;
    let value = parsed.get("value")?.as_str()?;
    let body = if value == definition.name.as_str() {
        "1"
    } else {
        value.strip_prefix(definition.name.as_str())?.strip_prefix('=')?
    };
    let mut line = format!("`define {}", definition.name);
    if !body.is_empty() {
        line.push(' ');
        line.push_str(body);
    }
    Some(line)
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
) -> Option<Markup> {
    macro_definitions_markup(db, anchor_file_id, std::slice::from_ref(definition))
}

pub(super) fn macro_definitions_markup(
    db: &RootDb,
    anchor_file_id: FileId,
    definitions: &[MacroDefinition],
) -> Option<Markup> {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        render_macro_definition_display(db, &mut markup, anchor_file_id, &definitions[0])?;
        return Some(markup);
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
    Some(markup)
}

fn render_macro_definition_display(
    db: &RootDb,
    markup: &mut Markup,
    anchor_file_id: FileId,
    definition: &MacroDefinition,
) -> Option<()> {
    let Some(line) = macro_definition_line(db, definition) else {
        return None;
    };
    let Some(source) = macro_definition_source_fact(db, definition, anchor_file_id) else {
        tracing::error!(
            macro_name = %definition.name,
            file_id = ?definition.file_id,
            "macro definition source location cannot be rendered"
        );
        return None;
    };
    markup.title(&macro_title(definition.name.as_str()));
    markup.push_with_code_fence(&line);
    markup.metadata_line(&format!("from {source}"));
    Some(())
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
