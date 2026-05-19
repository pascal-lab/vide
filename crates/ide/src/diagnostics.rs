use base_db::source_db::SourceRootDb;
use ide_db::root_db::RootDb;
use syntax::{DiagnosticSeverity, SyntaxDiagnostic};
use utils::text_edit::{TextRange, TextSize};
use vfs::FileId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    SlangParse,
    SlangSemantic,
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
    if source_root.is_ignored() {
        return Vec::new();
    }
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
    let source_root = db.source_root(source_root_id);
    if source_root.is_ignored() { vec![file_id] } else { source_root.iter().collect() }
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
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_db::SourceRootDb,
        source_root::{SourceRoot, SourceRootId},
    };
    use ide_db::root_db::RootDb;
    use triomphe::Arc;
    use utils::{lines::LineEnding, paths::AbsPathBuf, test_support::TestDir};
    use vfs::{ChangeKind, ChangedFile, FileId, FileSet, VfsPath};

    use super::{diagnostics, source_root_diagnostics};

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

    #[test]
    fn semantic_diagnostics_map_include_header_files() {
        let root = if cfg!(windows) {
            "C:/vizsla-diagnostics-include"
        } else {
            "/vizsla-diagnostics-include"
        };
        let root = AbsPathBuf::assert(root.into());
        let top_path = root.join("top.sv");
        let header_path = root.join("defs.vh");

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(top_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(header_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(
                Arc::from("module top;\n`include \"defs.vh\"\nendmodule\n"),
                LineEnding::Unix,
            ),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(
                Arc::from("logic value = missing_name;\n"),
                LineEnding::Unix,
            ),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig { predefines: Vec::new(), include_dirs: vec![root] },
            }],
        )));
        db.apply_change(change);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics.iter().any(|diag| diag.message.contains("missing_name")),
            "expected semantic diagnostic in included header: {diagnostics:?}"
        );
        assert!(
            diagnostics.iter().all(|diag| diag.file_id == FileId(1)),
            "header diagnostics should be attributed to the header file: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_do_not_compile_included_sv_as_root_source() {
        let dir = TestDir::new("diagnostics-included-sv");
        let root = dir.path().to_path_buf();
        let pkg_path = root.join("a_pkg.sv");
        let frag_path = root.join("z_frag.sv");
        let pkg_text = "module pkg_mod;\n`include \"z_frag.sv\"\nendmodule\n";
        let disk_frag_text = "logic value = 1'b0;\n";
        let vfs_frag_text = "logic value = missing_name;\n";
        std::fs::write(&pkg_path, pkg_text).unwrap();
        std::fs::write(&frag_path, disk_frag_text).unwrap();

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(pkg_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(frag_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(pkg_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(vfs_frag_text), LineEnding::Unix),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert!(plan.include_only.contains(&FileId(1)));
        assert_eq!(plan.roots, vec![FileId(0)]);

        let diagnostics = diagnostics(&db, FileId(1));

        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.file_id == FileId(1) && diag.message.contains("missing_name")),
            "included .sv should use VFS text and receive mapped diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_diagnostics_follow_transitive_included_sv_buffers() {
        let dir = TestDir::new("diagnostics-transitive-included-sv");
        let src_root = dir.join("src");
        let include_root = dir.join("include");
        std::fs::create_dir_all(&src_root).unwrap();
        std::fs::create_dir_all(&include_root).unwrap();

        let top_path = src_root.join("top.sv");
        let mid_path = include_root.join("mid.sv");
        let leaf_path = include_root.join("leaf.sv");
        let top_text = "module top;\n`include \"mid.sv\"\nendmodule\n";
        let mid_text = "`include \"leaf.sv\"\n";
        let disk_leaf_text = "logic value = 1'b0;\n";
        let vfs_leaf_text = "logic value = missing_name;\n";
        std::fs::write(&top_path, top_text).unwrap();
        std::fs::write(&mid_path, mid_text).unwrap();
        std::fs::write(&leaf_path, disk_leaf_text).unwrap();

        let mut db = RootDb::new(None);
        let mut src_files = FileSet::default();
        src_files.insert(FileId(0), VfsPath::from(top_path));
        let mut include_files = FileSet::default();
        include_files.insert(FileId(1), VfsPath::from(mid_path));
        include_files.insert(FileId(2), VfsPath::from(leaf_path));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(top_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(mid_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(2),
            change_kind: ChangeKind::Create(Arc::from(vfs_leaf_text), LineEnding::Unix),
        });
        change.set_roots(vec![
            SourceRoot::new_local(src_files),
            SourceRoot::new_local(include_files),
        ]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0)), None],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    predefines: Vec::new(),
                    include_dirs: vec![include_root],
                },
            }],
        )));
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert_eq!(plan.include_only.len(), 2);
        assert!(plan.include_only.contains(&FileId(1)));
        assert!(plan.include_only.contains(&FileId(2)));

        let diagnostics = source_root_diagnostics(&db, FileId(0));

        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.file_id == FileId(2) && diag.message.contains("missing_name")),
            "transitively included .sv should use VFS text: {diagnostics:?}"
        );
    }

    #[test]
    fn semantic_compilation_preloads_root_source_buffers() {
        let dir = TestDir::new("diagnostics-preloaded-roots");
        let root = dir.path().to_path_buf();
        let a_path = root.join("a.sv");
        let b_path = root.join("b.sv");
        let a_text = "module a; endmodule\n";
        let b_text = "module b; endmodule\n";
        std::fs::write(&a_path, a_text).unwrap();
        std::fs::write(&b_path, b_text).unwrap();

        let mut db = RootDb::new(None);
        let mut file_set = FileSet::default();
        file_set.insert(FileId(0), VfsPath::from(a_path.clone()));
        file_set.insert(FileId(1), VfsPath::from(b_path.clone()));

        let mut change = Change::new();
        change.add_changed_file(ChangedFile {
            file_id: FileId(0),
            change_kind: ChangeKind::Create(Arc::from(a_text), LineEnding::Unix),
        });
        change.add_changed_file(ChangedFile {
            file_id: FileId(1),
            change_kind: ChangeKind::Create(Arc::from(b_text), LineEnding::Unix),
        });
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        db.apply_change(change);

        let plan = db.compilation_plan_for_root(SourceRootId(0));
        assert_eq!(plan.roots, vec![FileId(0), FileId(1)]);
        let buffers = base_db::compilation_plan::compilation_source_buffers_for_plan(&db, &plan);
        let buffer_paths = buffers.iter().map(|buffer| buffer.path.as_str()).collect::<Vec<_>>();
        let a_path = a_path.to_string();
        let b_path = b_path.to_string();
        assert!(buffer_paths.contains(&a_path.as_str()));
        assert!(buffer_paths.contains(&b_path.as_str()));
    }
}
