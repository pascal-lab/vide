use hir::{
    base_db::{
        source_db::{SourceDb, SourceRootDb},
        source_root::SourceRootRole,
    },
    preproc::{MacroDefinition, MacroExpansionDefinition, MacroParamDefinition},
};
use vfs::FileId;

use crate::{db::root_db::RootDb, markup::Markup};

struct MacroSourceLink {
    label: String,
    target: String,
}

pub(super) fn render_macro_expansion_header(
    markup: &mut Markup,
    definition: &MacroExpansionDefinition,
) {
    match definition {
        MacroExpansionDefinition::Source(definition) => {
            markup.push_with_code_fence(&macro_signature(definition));
        }
        MacroExpansionDefinition::Builtin { name, .. } => {
            markup.push_with_code_fence(&format!("`{name}"));
        }
    }
}

pub(super) fn render_macro_expansion_separator(markup: &mut Markup) {
    markup.newline();
    markup.print(super::MACRO_EXPANSION_SEPARATOR);
    markup.newline();
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

fn macro_definition_line(definition: &MacroDefinition) -> String {
    let mut line = String::from("`define ");
    line.push_str(&macro_signature(definition));
    let body = macro_definition_body_text(definition);
    if !body.is_empty() {
        line.push(' ');
        line.push_str(&body);
    }
    line
}

fn macro_definition_source_link(
    db: &RootDb,
    definition: &MacroDefinition,
    anchor_file_id: FileId,
) -> Option<MacroSourceLink> {
    match &definition.source {
        hir::preproc::MappedPreprocSource::RealFile { file_id } => {
            macro_file_source_link(db, *file_id, anchor_file_id)
        }
        hir::preproc::MappedPreprocSource::VirtualFile { .. }
        | hir::preproc::MappedPreprocSource::VirtualDisplay { .. } => None,
    }
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

fn display_project_path(mut path: String) -> String {
    while path.starts_with('/') {
        path.remove(0);
    }
    display_hover_path(path)
}

fn display_hover_path(path: String) -> String {
    path.replace('\\', "/")
}

fn file_link_target(path: &str) -> String {
    let path = display_hover_path(path.to_owned());
    if path.starts_with('/') { format!("file://{path}") } else { format!("file:///{path}") }
}

pub(super) fn macro_param_definition_markup(definition: &MacroParamDefinition) -> Markup {
    macro_param_definitions_markup(std::slice::from_ref(definition))
}

pub(super) fn macro_param_definitions_markup(definitions: &[MacroParamDefinition]) -> Markup {
    let mut markup = Markup::new();
    if definitions.len() == 1 {
        markup.print("Macro parameter");
        markup.newline();
        markup.push_with_backticks(definitions[0].name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(definitions[0].macro_definition.name.as_str());
        return markup;
    }

    markup.print("Macro parameters");
    for definition in definitions {
        markup.newline();
        markup.push_with_backticks(definition.name.as_str());
        markup.print(" of ");
        markup.push_with_backticks(definition.macro_definition.name.as_str());
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

    markup.print("Macro definitions");
    for definition in definitions {
        markup.newline();
        markup.push_with_backticks(definition.name.as_str());
        if let Some(path) = db.file_path(definition.file_id) {
            markup.print(" ");
            markup.print(&path.to_string());
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
    markup.push_with_code_fence(&macro_definition_line(definition));
    render_macro_expansion_separator(markup);
    render_macro_source_link(db, markup, definition, anchor_file_id);
}

pub(super) fn render_macro_source_link(
    db: &RootDb,
    markup: &mut Markup,
    definition: &MacroDefinition,
    anchor_file_id: FileId,
) {
    let Some(source) = macro_definition_source_link(db, definition, anchor_file_id) else {
        return;
    };
    markup.print_with_strong("Macro");
    markup.print(" from [");
    markup.print(&markdown_link_label(&source.label));
    markup.print("](<");
    markup.print(&markdown_link_destination(&source.target));
    markup.print(">)");
}

fn markdown_link_label(label: &str) -> String {
    label.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]")
}

fn markdown_link_destination(destination: &str) -> String {
    destination.replace('>', "%3E")
}

fn macro_definition_body_text(definition: &MacroDefinition) -> String {
    definition.body_tokens.iter().map(|token| token.as_str()).collect::<Vec<_>>().join(" ")
}
