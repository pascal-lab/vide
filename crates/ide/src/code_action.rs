use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use syntax::ast::{AstNode, CompilationUnit};
use utils::text_edit::{TextRange, TextSize};
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
}

pub(crate) fn append_missing_list_entries(entries: Vec<String>, has_existing: bool) -> String {
    let mut text = entries.join(", ");
    if has_existing && !text.is_empty() {
        text.insert_str(0, ", ");
    }
    text
}

impl CodeActionDiagnostics {
    pub fn allows_repair(&self, repair: RepairKind) -> bool {
        self.items.is_empty() || self.items.iter().any(|diag| diag.allows_repair(repair))
    }
}

impl CodeActionDiagnostic {
    fn allows_repair(&self, repair: RepairKind) -> bool {
        match repair {
            RepairKind::MissingConnection => {
                self.source == Some(DiagnosticSource::Semantic)
                    && (matches!(
                            self.option.as_deref(),
                            Some("unconnected-port" | "unconnected-unnamed-port")
                        )
                        || self.code
                            == Some(DiagnosticCode { subsystem: 2, code: 260 })
                        || self.code
                            == Some(DiagnosticCode { subsystem: 2, code: 261 }))
            }
            RepairKind::MissingParameter => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.code == Some(DiagnosticCode { subsystem: 2, code: 29 })
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
    ) -> Self {
        let compilation_unit = sema.parse(file_id);
        Self { sema, file_id, range, diagnostics, compilation_unit }
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
    let ctx = CodeActionCtx::new(&sema, file_id, range, diagnostics);

    let mut collector = CodeActionCollector::new(&ctx, resolve_strategy);
    handlers::all().iter().for_each(|handler| {
        handler(&mut collector, &ctx);
    });

    collector.finish()
}

mod handlers {
    use super::{CodeActionCollector, CodeActionCtx};

    pub(crate) type Handler = fn(&mut CodeActionCollector, &CodeActionCtx<'_>) -> Option<()>;

    mod add_missing_connections;
    mod add_missing_parameters;

    pub(crate) fn all() -> &'static [Handler] {
        &[
            add_missing_connections::add_missing_connections,
            add_missing_parameters::add_missing_parameters,
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
        })?;
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
                option: Some("unconnected-port".to_owned()),
            },
            RepairKind::MissingParameter => CodeActionDiagnostic {
                source: Some(DiagnosticSource::Semantic),
                code: Some(DiagnosticCode { subsystem: 2, code: 29 }),
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

    #[test]
    fn missing_connection_repair_requires_matching_diagnostic() {
        let (db, file_id, offset) = db_with_file(
            "module child(input a, input b); endmodule\nmodule top; child u(/*caret*/.a()); endmodule\n",
        );
        let actions = code_action(
            &db,
            file_id,
            utils::text_edit::TextRange::empty(offset),
            CodeActionDiagnostics { items: vec![diagnostic_for_repair(RepairKind::MissingParameter)] },
            CodeActionResolveStrategy::All,
        );

        assert!(actions.iter().all(|action| action.id.name != "add_missing_connections"));
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
    fn missing_parameter_repair_fills_named_parameters() {
        let text = "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n";
        let fixed = apply_action(text, RepairKind::MissingParameter).unwrap();
        assert_eq!(
            fixed,
            "module child #(parameter A = 1, parameter B) (); endmodule\nmodule top; child #(.A(1), .B()) u(); endmodule\n"
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
    fn missing_parameter_repair_is_not_offered_when_nothing_is_missing() {
        let labels = action_labels(
            "module child #(parameter A = 1) (); endmodule\nmodule top; child #(/*caret*/.A(1)) u(); endmodule\n",
            RepairKind::MissingParameter,
        );
        assert!(!labels.iter().any(|label| label == "Fill parameters"));
    }
}
