use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootRole,
    },
    hir_def::macro_file::MacroExpansionDefinition,
    preproc::{MacroDefinition, MacroParamDefinition},
};
use vfs::FileId;

use super::expansion::macro_expansion_hover_text;
use crate::{
    db::{line_index_db::LineIndexDb, root_db::RootDb},
    markup::{
        Markup, display_hover_path, display_project_path, file_link_target, inline_code,
        markdown_link,
    },
};

struct MacroSourceLink {
    label: String,
    target: String,
}

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

fn macro_file_source_link(
    db: &RootDb,
    file_id: FileId,
    anchor_file_id: FileId,
) -> Option<MacroSourceLink> {
    let source_root = db.source_root(db.source_root_id(file_id));
    let label = if matches!(source_root.role(), SourceRootRole::Local)
        && let Some(label) = local_source_root_path_label(db, file_id, anchor_file_id)
    {
        label
    } else {
        source_root
            .path_for_file(&file_id)
            .map(|path| display_hover_path(path.to_string()))
            .or_else(|| db.file_path(file_id).map(|path| display_hover_path(path.to_string())))?
    };
    let target = db
        .file_path(file_id)
        .map(|path| file_link_target(&path.to_string()))
        .unwrap_or_else(|| label.clone());
    Some(MacroSourceLink { label, target })
}

fn local_source_root_path_label(
    db: &RootDb,
    file_id: FileId,
    anchor_file_id: FileId,
) -> Option<String> {
    let source_root = db.source_root(db.source_root_id(file_id));
    let source_path = source_root.path_for_file(&file_id)?;
    let Some(target_path) = source_path.as_abs_path() else {
        return Some(display_project_path(source_path.to_string()));
    };

    let anchor_source_root = db.source_root(db.source_root_id(anchor_file_id));
    let anchor_path = anchor_source_root.path_for_file(&anchor_file_id)?.as_abs_path()?;
    let mut common_dir = anchor_path.parent()?.to_path_buf();
    while !target_path.starts_with(common_dir.as_path()) {
        if !common_dir.pop() {
            return None;
        }
    }
    if !has_normal_path_component(common_dir.as_path()) {
        return None;
    }

    target_path
        .strip_prefix(common_dir.as_path())
        .map(|path| display_project_path(path.as_ref().display().to_string()))
}

fn has_normal_path_component(path: &utils::paths::AbsPath) -> bool {
    path.components().any(|component| matches!(component, utils::paths::Utf8Component::Normal(_)))
}

pub(super) fn macro_param_definition_markup(definition: &MacroParamDefinition) -> Markup {
    macro_param_definitions_markup(std::slice::from_ref(definition))
}

pub(super) fn macro_param_definitions_markup(definitions: &[MacroParamDefinition]) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        let definition = &definitions[0];
        markup.title(&format!("Macro parameter {}", inline_code(definition.name.as_str())));
        markup.section("Facts");
        markup.fact("Macro", &inline_code(definition.macro_definition.name.as_str()));
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
    markup.section("Facts");
    let source = macro_definition_source_fact(db, definition, anchor_file_id)
        .unwrap_or_else(|| "unavailable".to_string());
    markup.fact("Source", &source);
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
        MacroExpansionDefinition::Builtin { .. } => Some("Builtin".to_string()),
    }
}

fn macro_file_source_fact(
    db: &RootDb,
    file_id: FileId,
    offset: utils::line_index::TextSize,
    anchor_file_id: FileId,
) -> Option<String> {
    let source = macro_file_source_link(db, file_id, anchor_file_id)?;
    let line = db.line_index(file_id).try_line_col(offset)?.line + 1;
    Some(format!("{} line {line}", markdown_link(&source.label, &source.target)))
}
