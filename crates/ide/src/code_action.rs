use hir::{
    container::InContainer,
    hir_def::{
        declaration::Declaration,
        module::{Module, ModuleId, port::Ports},
    },
    semantics::Semantics,
};
use ide_db::root_db::RootDb;
use smol_str::SmolStr;
use syntax::ast::{AstNode, CompilationUnit};
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

use crate::source_change::{SourceChange, SourceChangeBuilder};

#[derive(Debug, Clone, Default)]
pub struct CodeActionDiagnostics {
    pub items: Vec<CodeActionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionDiagnostic {
    pub source: Option<DiagnosticSource>,
    pub code: Option<DiagnosticCode>,
    pub name: Option<String>,
    pub option: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    Parse,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCode {
    pub subsystem: u16,
    pub code: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    MissingConnection,
    MissingParameter,
    ConvertOrderedPorts,
    ConvertOrderedParams,
    RemoveEmptyPortConnections,
    AddImplicitNamedPortParens,
    AddInstanceParens,
}

pub(crate) struct MissingListEdit {
    pub range: TextRange,
    pub replacement: String,
}

pub(crate) fn port_names(module: &Module) -> Vec<SmolStr> {
    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            ports.values().filter_map(|port| port.label.clone()).collect()
        }
        Ports::Ansi(ports) => ports
            .values()
            .flat_map(|port| port.decls.clone())
            .filter_map(|decl| module.get(decl).name.clone())
            .collect(),
    }
}

pub(crate) fn remaining_ordered_port_names(module: &Module, connected: usize) -> Vec<SmolStr> {
    match &module.ports {
        Ports::NonAnsi { ports, .. } => {
            ports.values().skip(connected).filter_map(|port| port.label.clone()).collect()
        }
        Ports::Ansi(ports) => ports
            .values()
            .flat_map(|port| port.decls.clone())
            .skip(connected)
            .filter_map(|decl| module.get(decl).name.clone())
            .collect(),
    }
}

pub(crate) fn leading_parameter_names(module: &Module) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .take_while(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
        .flat_map(|declaration| declaration.decls())
        .filter_map(|decl| module.get(decl).name.clone())
        .collect()
}

pub(crate) fn all_parameter_names(module: &Module) -> Vec<SmolStr> {
    module
        .declarations
        .values()
        .filter(|declaration| matches!(declaration, Declaration::ParamDecl(_)))
        .flat_map(|declaration| declaration.decls())
        .filter_map(|decl| module.get(decl).name.clone())
        .collect()
}

pub(crate) fn missing_member_entry_text(
    sema: &Semantics<'_, RootDb>,
    module_id: ModuleId,
    name: SmolStr,
    is_ordered: bool,
    unresolved_ordered_value: &str,
) -> String {
    match (sema.name_to_def(InContainer::new(module_id.into(), name.clone())), is_ordered) {
        (None, true) => format!("/* {name} */ {unresolved_ordered_value}"),
        (None, false) => format!(".{name}()"),
        (Some(_), true) => name.to_string(),
        (Some(_), false) => format!(".{name}"),
    }
}

pub(crate) fn apply_missing_list_edit(
    builder: &mut SourceChangeBuilder,
    text: &str,
    open_paren: TextRange,
    close_paren: TextRange,
    item_ranges: impl IntoIterator<Item = TextRange>,
    entries: Vec<String>,
) {
    if let Some(edit) = missing_list_edit(text, open_paren, close_paren, item_ranges, entries) {
        builder.replace(edit.range, edit.replacement);
    }
}

pub(crate) fn missing_list_edit(
    text: &str,
    open_paren: TextRange,
    close_paren: TextRange,
    item_ranges: impl IntoIterator<Item = TextRange>,
    entries: Vec<String>,
) -> Option<MissingListEdit> {
    if entries.is_empty() {
        return None;
    }

    let open_end = open_paren.end();
    let close_start = close_paren.start();
    if close_start < open_end {
        return None;
    }

    let open_end_usize = usize::from(open_end);
    let close_start_usize = usize::from(close_start);
    let content = text.get(open_end_usize..close_start_usize)?;
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let multiline = content.contains('\n');

    let trimmed_len = content.trim_end_matches(char::is_whitespace).len();
    let trimmed = &content[..trimmed_len];
    let trailing_comma = trimmed.ends_with(',');
    let meaningful_len =
        if trailing_comma { trimmed.len().saturating_sub(1) } else { trimmed.len() };
    let has_existing_text = !trimmed[..meaningful_len].trim().is_empty();

    let last_token_end = TextSize::from((open_end_usize + trimmed.len()) as u32);
    let range_start = if trailing_comma {
        last_token_end
    } else if has_existing_text {
        TextSize::from((open_end_usize + meaningful_len) as u32)
    } else {
        open_end
    };
    let range = TextRange::new(range_start, close_start);

    let replacement = if multiline {
        let close_indent = line_indent(text, close_start);
        let item_indent = item_ranges
            .into_iter()
            .filter(|range| !range.is_empty() && range.start() < close_start)
            .last()
            .and_then(|range| item_line_indent(text, range.start()))
            .unwrap_or_else(|| format!("{close_indent}    "));

        let mut lines = Vec::new();
        let entries_len = entries.len();
        for (idx, entry) in entries.into_iter().enumerate() {
            let needs_comma = trailing_comma || idx + 1 < entries_len;
            let comma = if needs_comma { "," } else { "" };
            lines.push(format!("{item_indent}{entry}{comma}"));
        }

        let rendered_entries = lines.join(newline);
        let prefix = if has_existing_text && !trailing_comma { "," } else { "" };
        format!("{prefix}{newline}{rendered_entries}{newline}{close_indent}")
    } else {
        let rendered_entries = entries.join(", ");
        if has_existing_text {
            let separator = if trailing_comma { " " } else { ", " };
            format!("{separator}{rendered_entries}")
        } else {
            rendered_entries
        }
    };

    Some(MissingListEdit { range, replacement })
}

fn line_indent(text: &str, offset: TextSize) -> String {
    let offset = usize::from(offset).min(text.len());
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    text[line_start..offset].chars().take_while(|ch| *ch == ' ' || *ch == '\t').collect()
}

fn item_line_indent(text: &str, offset: TextSize) -> Option<String> {
    let offset = usize::from(offset).min(text.len());
    let line_start = text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let before_item = &text[line_start..offset];
    before_item.chars().all(|ch| ch == ' ' || ch == '\t').then(|| before_item.to_owned())
}

impl CodeActionDiagnostics {
    pub fn allows_repair(&self, repair: RepairKind) -> bool {
        self.items.iter().any(|diag| diag.allows_repair(repair))
    }
}

impl CodeActionDiagnostic {
    pub fn allows_repair(&self, repair: RepairKind) -> bool {
        match repair {
            RepairKind::MissingConnection => {
                self.source == Some(DiagnosticSource::Semantic)
                    && (matches!(
                        self.option.as_deref(),
                        Some("unconnected-port" | "unconnected-unnamed-port")
                    ) || self.code == Some(DiagnosticCode { subsystem: 2, code: 260 })
                        || self.code == Some(DiagnosticCode { subsystem: 2, code: 261 }))
            }
            RepairKind::MissingParameter => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("ParamHasNoValue")
            }
            RepairKind::ConvertOrderedPorts => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedPorts")
            }
            RepairKind::ConvertOrderedParams => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedParams")
            }
            RepairKind::RemoveEmptyPortConnections => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedPorts")
            }
            RepairKind::AddImplicitNamedPortParens => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("ImplicitNamedPortNotFound")
            }
            RepairKind::AddInstanceParens => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("InstanceMissingParens")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum CodeActionResolveStrategy {
    None,
    All,
    Single { name: String },
}

impl CodeActionResolveStrategy {
    pub fn is_none(&self) -> bool {
        matches!(self, CodeActionResolveStrategy::None)
    }

    pub fn should_resolve(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::All => true,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }

    pub fn should_add(&self, id: CodeActionId) -> bool {
        match self {
            CodeActionResolveStrategy::All | CodeActionResolveStrategy::None => false,
            CodeActionResolveStrategy::Single { name } => id.name == name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeActionId {
    pub name: &'static str,
    pub kind: CodeActionKind,
    /// Diagnostic repair this action can satisfy when a matching diagnostic is
    /// present.
    pub repair: Option<RepairKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Generate,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
}

impl CodeActionKind {
    pub fn contains(self, other: CodeActionKind) -> bool {
        if self == other {
            return true;
        }

        match self {
            CodeActionKind::Generate => true,
            CodeActionKind::Refactor => matches!(
                other,
                CodeActionKind::RefactorExtract
                    | CodeActionKind::RefactorInline
                    | CodeActionKind::RefactorRewrite
            ),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeAction {
    pub id: CodeActionId,
    pub label: String,
    /// Target ranges are used to sort assists: the smaller the target range,
    /// the more specific assist is, and so it should be sorted first.
    pub target: TextRange,
    /// Compute it lazily
    pub source_change: Option<SourceChange>,
}

pub(crate) struct CodeActionCollector {
    file: FileId,
    resolve_strategy: CodeActionResolveStrategy,
    buf: Vec<CodeAction>,
}

impl CodeActionCollector {
    fn new(ctx: &CodeActionCtx, resolve_strategy: CodeActionResolveStrategy) -> Self {
        Self { file: ctx.file_id, resolve_strategy, buf: Vec::new() }
    }

    pub(crate) fn add(
        &mut self,
        id: CodeActionId,
        label: impl Into<String>,
        target: TextRange,
        f: impl FnOnce(&mut SourceChangeBuilder),
    ) -> Option<()> {
        let source_change = if self.resolve_strategy.should_resolve(id) {
            let mut builder = SourceChangeBuilder::new(self.file);
            f(&mut builder);
            Some(builder.finish())
        } else {
            None
        };

        self.buf.push(CodeAction { id, label: label.into(), target, source_change });
        Some(())
    }

    fn finish(mut self) -> Vec<CodeAction> {
        self.buf.sort_by_key(|assist| assist.target.len());
        self.buf
    }
}

struct CodeActionCtx<'a> {
    sema: &'a Semantics<'a, RootDb>,
    file_id: FileId,
    range: TextRange,
    diagnostics: CodeActionDiagnostics,
    compilation_unit: CompilationUnit<'a>,
}

impl<'a> CodeActionCtx<'a> {
    fn new(
        sema: &'a Semantics<'a, RootDb>,
        file_id: FileId,
        range: TextRange,
        diagnostics: CodeActionDiagnostics,
    ) -> Option<Self> {
        let compilation_unit = CompilationUnit::cast(sema.parse_root(file_id)?)?;
        Some(Self { sema, file_id, range, diagnostics, compilation_unit })
    }

    fn offset(&self) -> TextSize {
        self.range.start()
    }

    fn find_node_at_offset<N: AstNode<'a>>(&self) -> Option<N> {
        self.sema.find_node_at_offset(self.compilation_unit.syntax(), self.offset())
    }
}

pub(crate) fn code_action(
    db: &RootDb,
    file_id: FileId,
    range: TextRange,
    diagnostics: CodeActionDiagnostics,
    resolve_strategy: CodeActionResolveStrategy,
) -> Vec<CodeAction> {
    let sema = Semantics::new(db);
    let Some(ctx) = CodeActionCtx::new(&sema, file_id, range, diagnostics) else {
        return Vec::new();
    };

    let mut collector = CodeActionCollector::new(&ctx, resolve_strategy);
    handlers::all().iter().for_each(|handler| {
        handler(&mut collector, &ctx);
    });

    collector.finish()
}

mod handlers {
    use super::{CodeActionCollector, CodeActionCtx};

    pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

    mod add_implicit_named_port_parens;
    mod add_instance_parens;
    mod add_missing_connections;
    mod add_missing_parameters;
    mod convert_ordered_connections;
    mod remove_empty_port_connections;

    pub(crate) fn all() -> &'static [Handler] {
        &[
            add_missing_connections::add_missing_connections,
            add_missing_parameters::add_missing_parameters,
            convert_ordered_connections::convert_ordered_ports,
            convert_ordered_connections::convert_ordered_params,
            remove_empty_port_connections::remove_empty_port_connections,
            add_implicit_named_port_parens::add_implicit_named_port_parens,
            add_instance_parens::add_instance_parens,
        ]
    }
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_root::SourceRoot};
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::{lines::LineEnding, text_edit::TextSize};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::*;

    fn db_with_file(text: &str) -> (RootDb, FileId, TextSize) {
        let marker = "/*caret*/";
        let offset = text.find(marker).expect("missing caret marker");
        let text = text.replace(marker, "");
        let file_id = FileId(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/test.sv".to_owned()));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile {
            file_id,
            change_kind: ChangeKind::Create(Arc::from(text.as_str()), LineEnding::Unix),
        });

        let mut db = RootDb::new(None);
        db.apply_change(change);
        (db, file_id, TextSize::from(offset as u32))
    }

    fn apply_action(text: &str, repair: RepairKind) -> Option<String> {
        let (db, file_id, offset) = db_with_file(text);
        let diagnostics = CodeActionDiagnostics { items: vec![diagnostic_for_repair(repair)] };
        let actions = code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            diagnostics,
            CodeActionResolveStrategy::All,
        );
        let action = actions.into_iter().find(|action| match repair {
            RepairKind::MissingConnection => action.id.name == "add_missing_connections",
            RepairKind::MissingParameter => action.id.name == "add_missing_parameters",
            RepairKind::ConvertOrderedPorts => action.id.name == "convert_ordered_ports",
            RepairKind::ConvertOrderedParams => action.id.name == "convert_ordered_params",
            RepairKind::RemoveEmptyPortConnections => {
                action.id.name == "remove_empty_port_connections"
            }
            RepairKind::AddImplicitNamedPortParens => {
                action.id.name == "add_implicit_named_port_parens"
            }
            RepairKind::AddInstanceParens => action.id.name == "add_instance_parens",
        })?;
        let mut text = text.replace("/*caret*/", "");
        let edit = action.source_change?.text_edits.remove(&file_id)?;
        edit.apply(&mut text);
        Some(text)
    }

    fn apply_action_without_diagnostics(text: &str, action_name: &str) -> Option<String> {
        let (db, file_id, offset) = db_with_file(text);
        let actions = code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            CodeActionDiagnostics::default(),
            CodeActionResolveStrategy::All,
        );
        let action = actions.into_iter().find(|action| action.id.name == action_name)?;
        let mut text = text.replace("/*caret*/", "");
        let edit = action.source_change?.text_edits.remove(&file_id)?;
        edit.apply(&mut text);
        Some(text)
    }

    fn diagnostic_for_repair(repair: RepairKind) -> CodeActionDiagnostic {
        match repair {
            RepairKind::MissingConnection => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("UnconnectedNamedPort".to_owned()),
                option: Some("unconnected-port".to_owned()),
            },
            RepairKind::MissingParameter => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: Some(DiagnosticCode { subsystem: 2, code: 29 }),
                name: Some("ParamHasNoValue".to_owned()),
                option: None,
            },
            RepairKind::ConvertOrderedPorts => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("MixingOrderedAndNamedPorts".to_owned()),
                option: None,
            },
            RepairKind::ConvertOrderedParams => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("MixingOrderedAndNamedParams".to_owned()),
                option: None,
            },
            RepairKind::RemoveEmptyPortConnections => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("MixingOrderedAndNamedPorts".to_owned()),
                option: None,
            },
            RepairKind::AddImplicitNamedPortParens => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("ImplicitNamedPortNotFound".to_owned()),
                option: None,
            },
            RepairKind::AddInstanceParens => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: None,
                name: Some("InstanceMissingParens".to_owned()),
                option: None,
            },
        }
    }

    fn action_labels(text: &str, repair: RepairKind) -> Vec<String> {
        let (db, file_id, offset) = db_with_file(text);
        let diagnostics = CodeActionDiagnostics { items: vec![diagnostic_for_repair(repair)] };
        code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            diagnostics,
            CodeActionResolveStrategy::None,
        )
        .into_iter()
        .map(|action| action.label)
        .collect()
    }

    fn action_labels_without_diagnostics(text: &str) -> Vec<String> {
        let (db, file_id, offset) = db_with_file(text);
        code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            CodeActionDiagnostics::default(),
            CodeActionResolveStrategy::None,
        )
        .into_iter()
        .map(|action| action.label)
        .collect()
    }

    #[test]
    fn missing_connection_repair_requires_matching_diagnostic() {
        let (db, file_id, offset) = db_with_file(
            "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n",
        );
        let actions = code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            CodeActionDiagnostics {
                items: vec![diagnostic_for_repair(RepairKind::MissingParameter)],
            },
            CodeActionResolveStrategy::All,
        );

        assert!(actions.iter().all(|action| action.id.name != "add_missing_connections"));
    }

    #[test]
    fn repair_actions_require_diagnostics() {
        let labels = action_labels_without_diagnostics(
            "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n",
        );

        assert!(!labels.iter().any(|label| label == "Fill connections"));
        assert!(!labels.iter().any(|label| label == "Fill parameters"));
    }

    #[test]
    fn convert_ordered_ports_is_available_without_diagnostics() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/x, y); endmodule\n";
        let fixed = apply_action_without_diagnostics(text, "convert_ordered_ports").unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
        );
    }

    #[test]
    fn convert_ordered_params_is_available_without_diagnostics() {
        let text = "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/8, 16) u(); endmodule\n";
        let fixed = apply_action_without_diagnostics(text, "convert_ordered_params").unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(.A(8), .B(16)) u(); endmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_fills_named_connections() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_handles_one_line_trailing_comma() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a(),); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_preserves_multiline_named_style() {
        let text = "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    /*caret*/.a()\n);\nendmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    .a(),\n    .b(),\n    .c()\n);\nendmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_preserves_multiline_trailing_comma_style() {
        let text = "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    /*caret*/.a(),\n);\nendmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b, input c); endmodule\nmodule top;\nchild u(\n    .a(),\n    .b(),\n    .c(),\n);\nendmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_fills_empty_named_connection_list() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(), .b()); endmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_fills_ordered_connections() {
        let text = "module child(input a, input b, input c); endmodule\nmodule top; logic b, c; child u(/*caret*/1'b0); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b, input c); endmodule\nmodule top; logic b, c; child u(1'b0, b, c); endmodule\n"
        );
    }

    #[test]
    fn missing_connection_repair_uses_valid_ordered_placeholders() {
        let text = "module child(input a, input b, input c); endmodule\nmodule top; child u(/*caret*/1'b0); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingConnection).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b, input c); endmodule\nmodule top; child u(1'b0, /* b */ '0, /* c */ '0); endmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_fills_named_parameters() {
        let text = "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(.A(1), .B()) u(); endmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_preserves_multiline_trailing_comma_style() {
        let text = "module child #(parameter A = 1, parameter B, parameter C) (); endmodule\nmodule top;\nchild #(\n    /*caret*/.A(1),\n) u();\nendmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A = 1, parameter B, parameter C) (); endmodule\nmodule top;\nchild #(\n    .A(1),\n    .B(),\n    .C(),\n) u();\nendmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_fills_empty_parameter_list() {
        let text = "module child #(parameter A, parameter B) (); endmodule\nmodule top; child #(/*caret*/) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A, parameter B) (); endmodule\nmodule top; child #(.A(), .B()) u(); endmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_fills_ordered_parameters() {
        let text = "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; parameter B = 2; parameter C = 3; child #(/*caret*/1) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; parameter B = 2; parameter C = 3; child #(1, B, C) u(); endmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_uses_valid_ordered_placeholders() {
        let text = "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; child #(/*caret*/1) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A, parameter B, parameter C) (); endmodule\nmodule top; child #(1, /* B */ 0, /* C */ 0) u(); endmodule\n"
        );
    }

    #[test]
    fn missing_parameter_repair_is_not_offered_when_nothing_is_missing() {
        let labels = action_labels(
            "module child #(parameter A = 1) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n",
            RepairKind::MissingParameter,
        );
        assert!(!labels.iter().any(|label| label == "Fill parameters"));
    }

    #[test]
    fn convert_ordered_ports_repair_names_ordered_connections() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/x, .b(y)); endmodule\n";
        let fixed = apply_action(text, RepairKind::ConvertOrderedPorts).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
        );
    }

    #[test]
    fn remove_empty_port_connection_repair_removes_trailing_comma() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y),/*caret*/); endmodule\n";
        let fixed = apply_action(text, RepairKind::RemoveEmptyPortConnections).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
        );
    }

    #[test]
    fn remove_empty_port_connection_repair_removes_middle_empty_connection() {
        let text = "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a(x), , .b(y)); endmodule\n";
        let fixed = apply_action(text, RepairKind::RemoveEmptyPortConnections).unwrap();
        assert_eq!(
            fixed,
            "module child(input a, input b); endmodule\nmodule top; child u(.a(x), .b(y)); endmodule\n"
        );
    }

    #[test]
    fn convert_ordered_params_repair_names_ordered_assignments() {
        let text = "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(/*caret*/8, .B(16)) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::ConvertOrderedParams).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A = 1, parameter B = 2) (); endmodule\nmodule top; child #(.A(8), .B(16)) u(); endmodule\n"
        );
    }

    #[test]
    fn implicit_named_port_repair_adds_empty_parens() {
        let text =
            "module child(input a); endmodule\nmodule top; child u(/*caret*/.a); endmodule\n";
        let fixed = apply_action(text, RepairKind::AddImplicitNamedPortParens).unwrap();
        assert_eq!(
            fixed,
            "module child(input a); endmodule\nmodule top; child u(.a()); endmodule\n"
        );
    }

    #[test]
    fn instance_missing_parens_repair_adds_port_list() {
        let text = "module child; endmodule\nmodule top; child u/*caret*/; endmodule\n";
        let fixed = apply_action(text, RepairKind::AddInstanceParens).unwrap();
        assert_eq!(fixed, "module child; endmodule\nmodule top; child u(); endmodule\n");
    }
}
