use base_db::source_db::{SourceDb, SourceRootDb};
use hir::{
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::BlockId,
        module::ModuleId,
        opaque::{OpaqueItemId, OpaqueItemSrc},
        stmt::{Stmt, StmtKind},
        subroutine::SubroutineSourceMap,
    },
    source_map::{IsNamedSrc, IsSrc},
};
use ide_db::root_db::RootDb;
use la_arena::Arena;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};
use vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    SlangParse,
    SlangSemantic,
    VizslaModel,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub file_id: FileId,
    pub code: u16,
    pub subsystem: u16,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub source: DiagnosticSource,
    pub range: TextRange,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

pub(crate) fn parse_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    db.parse_diagnostics(file_id)
        .iter()
        .map(|diag| Diagnostic {
            file_id,
            code: diag.code,
            subsystem: diag.subsystem,
            name: diag.name.clone(),
            option_name: diag.option_name.clone(),
            groups: diag.groups.clone(),
            source: DiagnosticSource::SlangParse,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        })
        .collect()
}

pub(crate) fn diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = parse_diagnostics(db, file_id);

    diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().filter_map(
        |(diag_file_id, diag)| {
            (*diag_file_id == file_id).then_some(Diagnostic {
                file_id: *diag_file_id,
                code: diag.code,
                subsystem: diag.subsystem,
                name: diag.name.clone(),
                option_name: diag.option_name.clone(),
                groups: diag.groups.clone(),
                source: DiagnosticSource::SlangSemantic,
                range: to_text_range(diag),
                severity: diag.severity,
                message: diag.message.clone(),
            })
        },
    ));

    diagnostics
}

pub(crate) fn source_root_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let source_root_id = db.source_root_id(file_id);
    let source_root = db.source_root(source_root_id);
    let mut diagnostics = Vec::new();

    for file_id in source_root.iter() {
        diagnostics.extend(parse_diagnostics(db, file_id));
    }

    diagnostics.extend(db.source_root_semantic_diagnostics(file_id).iter().map(
        |(diag_file_id, diag)| Diagnostic {
            file_id: *diag_file_id,
            code: diag.code,
            subsystem: diag.subsystem,
            name: diag.name.clone(),
            option_name: diag.option_name.clone(),
            groups: diag.groups.clone(),
            source: DiagnosticSource::SlangSemantic,
            range: to_text_range(diag),
            severity: diag.severity,
            message: diag.message.clone(),
        },
    ));

    diagnostics
}

pub(crate) fn source_root_file_ids(db: &RootDb, file_id: FileId) -> Vec<FileId> {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).iter().collect()
}

pub(crate) fn model_limit_diagnostics(db: &RootDb, file_id: FileId) -> Vec<Diagnostic> {
    let file_id = HirFileId(file_id);
    let (hir_file, file_src_map) = db.hir_file_with_source_map(file_id);
    let mut diagnostics = Vec::new();

    collect_opaque_diagnostics(file_id.file_id(), &file_src_map.opaque_srcs, &mut diagnostics);

    for (local_module_id, _) in hir_file.modules.iter() {
        let module_id = ModuleId::new(file_id, local_module_id);
        let (module, module_src_map) = db.module_with_source_map(module_id);
        collect_opaque_diagnostics(
            file_id.file_id(),
            &module_src_map.opaque_srcs,
            &mut diagnostics,
        );

        for (_, subroutine_source_map) in module.subroutine_source_maps.iter() {
            collect_subroutine_opaque_diagnostics(file_id, subroutine_source_map, &mut diagnostics);
        }

        for (_, proc) in module.procs.iter() {
            let mut block_ids = Vec::new();
            collect_stmt_block_ids(&module.stmts, proc.stmt, &mut block_ids);
            for block_id in block_ids {
                collect_block_opaque_diagnostics(db, block_id, &mut diagnostics);
            }
        }
    }

    diagnostics
}

fn collect_subroutine_opaque_diagnostics(
    file_id: HirFileId,
    source_map: &SubroutineSourceMap,
    diagnostics: &mut Vec<Diagnostic>,
) {
    collect_opaque_diagnostics(file_id.file_id(), &source_map.opaque_srcs, diagnostics);
}

fn collect_block_opaque_diagnostics(
    db: &RootDb,
    block_id: BlockId,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (block, source_map) = db.block_with_source_map(block_id);
    collect_opaque_diagnostics(block_id.file_id(db), &source_map.opaque_srcs, diagnostics);

    for (_, stmt) in block.stmts.iter() {
        if let StmtKind::Block(info) = &stmt.kind {
            collect_block_opaque_diagnostics(db, info.block_id, diagnostics);
        }
    }
}

fn collect_stmt_block_ids(
    stmts: &Arena<Stmt>,
    stmt_id: hir::hir_def::stmt::StmtId,
    out: &mut Vec<BlockId>,
) {
    let stmt = stmts.get(stmt_id);
    match &stmt.kind {
        StmtKind::Block(info) => out.push(info.block_id),
        StmtKind::TimingCtrl(_, stmt)
        | StmtKind::Forever(stmt)
        | StmtKind::DoWhile(stmt, _)
        | StmtKind::While(_, stmt)
        | StmtKind::Wait(_, stmt) => collect_stmt_block_ids(stmts, *stmt, out),
        StmtKind::For { stmt, .. } => collect_stmt_block_ids(stmts, *stmt, out),
        StmtKind::Cond { then_stmt, else_stmt, .. } => {
            collect_stmt_block_ids(stmts, *then_stmt, out);
            if let Some(else_stmt) = else_stmt {
                collect_stmt_block_ids(stmts, *else_stmt, out);
            }
        }
        StmtKind::Case { items, .. } => {
            for item in items {
                match item {
                    hir::hir_def::stmt::CaseItem::Case { clause, .. }
                    | hir::hir_def::stmt::CaseItem::Default(clause) => {
                        collect_stmt_block_ids(stmts, *clause, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn collect_opaque_diagnostics(
    file_id: FileId,
    source_map: &hir::source_map::SourceMap<OpaqueItemSrc, hir::hir_def::opaque::OpaqueItem>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (opaque_id, src) in source_map.iter() {
        diagnostics.push(model_limit_diagnostic(file_id, opaque_id, *src));
    }
}

fn model_limit_diagnostic(
    file_id: FileId,
    _opaque_id: OpaqueItemId,
    src: OpaqueItemSrc,
) -> Diagnostic {
    Diagnostic {
        file_id,
        code: 1,
        subsystem: 1,
        name: "VerilogModelUnsupportedConstruct".to_owned(),
        option_name: Some("verilog.modelUnsupportedConstructs".to_owned()),
        groups: vec!["verilog".to_owned(), "model-limitation".to_owned()],
        source: DiagnosticSource::VizslaModel,
        range: src.name_range().unwrap_or_else(|| src.range()),
        severity: DiagnosticSeverity::Warning,
        message: format!(
            "recognized Verilog-2005 construct; semantic IDE support is limited (kind: {:?})",
            src.kind
        ),
    }
}

fn to_text_range(diag: &SyntaxDiagnostic) -> TextRange {
    fn to_text_size(value: usize) -> TextSize {
        let raw = u32::try_from(value).unwrap_or(u32::MAX);
        TextSize::new(raw)
    }

    if let Some(range) = diag.primary_range.as_ref() {
        TextRange::new(to_text_size(range.start), to_text_size(range.end))
    } else if let Some(offset) = diag.location {
        let pos = to_text_size(offset);
        TextRange::new(pos, pos)
    } else {
        TextRange::empty(TextSize::new(0))
    }
}

#[cfg(test)]
mod tests {
    use base_db::{change::Change, source_db::SourceRootDb, source_root::SourceRoot};
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::lines::LineEnding;
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::diagnostics;

    fn db_with_files(files: &[(&str, &str)]) -> RootDb {
        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        let mut change = Change::new();

        for (idx, (path, text)) in files.iter().enumerate() {
            let file_id = FileId(idx as u32);
            let path = VfsPath::new_virtual_path((*path).to_owned());
            file_set.insert(file_id, path);
            change.add_changed_file(ChangedFile {
                file_id,
                change_kind: ChangeKind::Create(Arc::from(*text), LineEnding::Unix),
            });
        }

        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);
        db
    }

    #[test]
    fn semantic_diagnostics_include_other_workspace_files() {
        let db = db_with_files(&[
            ("/child.sv", "module child(input logic a, input logic b);\nendmodule\n"),
            ("/top.sv", "module top;\n  logic sig;\n  child u(.a(sig));\nendmodule\n"),
        ]);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("port 'b' has no connection")),
            "expected semantic diagnostic from module declared in another file: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "document diagnostics should only include diagnostics attributed to the requested file: {diagnostics:?}"
        );
        assert!(
            db.semantic_diagnostics(FileId(0)).is_empty(),
            "child file should not receive diagnostics that belong to top.sv"
        );
    }
}
