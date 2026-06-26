use std::{ffi::OsStr, fs, io::Cursor, path::PathBuf, process::Command};

use crossbeam_channel::unbounded;
use hir::base_db::compilation_plan::CompilationPlan;
use lsp_types::{
    Diagnostic, DiagnosticClientCapabilities, DiagnosticSeverity,
    DiagnosticWorkspaceClientCapabilities, NumberOrString, Position, Range,
    TextDocumentClientCapabilities, TraceValue, WorkspaceClientCapabilities, request::Request,
};
use project_model::project_manifest::ProjectManifestFileName;
use utils::{cancellation::CancellationToken, paths::AbsPathBuf, test_support::TestDir};
use vfs::FileId;

use super::{
    QIHE_OPTIONS_RUN_PATH, QiheCompileInput, QiheCompileInputSource, QiheLogSink, QiheRunId,
    QiheRunPlan, QiheUpdate, command_line, has_compile_mode, join_command_output, parse_source_loc,
    prepare_qihe_compile_command, prepare_qihe_run_command, qihe_compile_input_from_plan,
    qihe_progress_token, qihe_working_directory, resolve_qihe_run_plan, split_compile_args,
    stream_command_output, strip_ansi,
};
use crate::{
    Opt,
    config::{
        self,
        user_config::{QiheConfig, UserConfig},
    },
    global_state::{
        GlobalState, QiheDiagnosticState,
        task::{QiheTask, Task},
    },
    i18n::I18n,
};

fn new_test_state(name: &str) -> (TestDir, GlobalState) {
    let root = TestDir::new(name);
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        lsp_types::ClientCapabilities::default(),
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, _client) = lsp_server::Connection::memory();
    (root, GlobalState::new(server.sender, config, TraceValue::Off))
}

#[test]
fn parses_line_col_only_locations() {
    let loc = parse_source_loc("12:34").expect("location");
    assert_eq!((loc.file_name.as_deref(), loc.line, loc.column), (None, 12, 34));
}

#[test]
fn parses_file_locations_with_colons() {
    let loc = parse_source_loc("/tmp/a:b.sv:12:34").expect("location");
    assert_eq!((loc.file_name.as_deref(), loc.line, loc.column), (Some("/tmp/a:b.sv"), 12, 34));
}

#[test]
fn ignores_symbolic_locations() {
    for raw in ["@buggy", "#SourceUnknown"] {
        assert!(parse_source_loc(raw).is_none());
    }
}

#[test]
fn strips_ansi_escape_sequences() {
    assert_eq!(strip_ansi("\u{1b}[32mINFO\u{1b}[m hello"), "INFO hello");
}

#[test]
fn command_line_includes_cwd_program_and_arguments() {
    let cwd = if cfg!(windows) { "C:/repo with space" } else { "/repo with space" };
    let mut command = Command::new("qihe");
    command.current_dir(cwd).arg("compile").arg("rtl/top module.sv");

    let rendered = command_line(&command);

    assert!(rendered.contains("cwd="));
    assert!(rendered.contains("qihe"));
    assert!(rendered.contains("compile"));
    assert!(rendered.contains("\"rtl/top module.sv\""));
}

#[test]
fn qihe_working_directory_uses_normal_windows_path() {
    let cwd = std::env::current_dir().expect("current dir");
    let root = AbsPathBuf::assert_utf8(cwd.clone());

    let resolved = qihe_working_directory(Some(cwd), root.as_path());

    assert!(resolved.is_absolute());
    if cfg!(windows) {
        let rendered = resolved.to_string_lossy().replace('\\', "/");
        assert!(!rendered.starts_with("//?/"), "{rendered}");
    }
}

#[test]
fn command_output_streamer_strips_ansi_and_logs_lines() {
    let (sender, receiver) = unbounded();
    let sink = QiheLogSink::new(sender, QiheRunId::new(1), "test-token".to_owned());
    let handle = stream_command_output(
        Cursor::new("\u{1b}[32mfirst\u{1b}[m\nsecond\n".as_bytes().to_vec()),
        "qihe run".to_owned(),
        "stdout",
        sink,
    );

    let output = join_command_output(Some(handle));

    assert_eq!(output, "first\nsecond\n");
    let messages = receiver
        .try_iter()
        .filter_map(|task| match task {
            Task::Qihe(QiheTask::Log { message, .. }) => Some(message),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(messages, ["qihe run stdout: first\nqihe run stdout: second"]);
}

#[test]
fn stale_qihe_result_does_not_replace_current_diagnostics() {
    let root = TestDir::new("stale-qihe-result");
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        lsp_types::ClientCapabilities::default(),
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, _client) = lsp_server::Connection::memory();
    let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
    state.qihe.run_generation = QiheRunId::new(2);
    let file_id = FileId(0);
    let current = Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 1)),
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("qihe".to_owned()),
        message: "current".to_owned(),
        ..Diagnostic::default()
    };
    let stale = Diagnostic { message: "stale".to_owned(), ..current.clone() };
    let freshness = state.diagnostic_publish_freshness().commit();
    state.qihe.diagnostics.lock().insert(
        file_id,
        QiheDiagnosticState { freshness, generation: 1, diagnostics: vec![current.clone()] },
    );

    state.handle_qihe_task(QiheTask::Finished {
        run_id: QiheRunId::new(1),
        update: QiheUpdate {
            by_file: rustc_hash::FxHashMap::from_iter([(file_id, vec![stale])]),
            summary: "old run".to_owned(),
            freshness,
        },
        progress_token: "old".to_owned(),
    });

    let stored = state.qihe.diagnostics.lock().get(&file_id).unwrap().diagnostics.clone();
    assert_eq!(stored, vec![current]);
}

#[test]
fn current_qihe_result_closes_active_progress() {
    let root = TestDir::new("current-qihe-progress");
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        lsp_types::ClientCapabilities::default(),
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, _client) = lsp_server::Connection::memory();
    let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
    state.qihe.run_generation = QiheRunId::new(1);
    state.qihe.active_progress_token = Some("current".to_owned());

    state.handle_qihe_task(QiheTask::Finished {
        run_id: QiheRunId::new(1),
        update: QiheUpdate {
            by_file: rustc_hash::FxHashMap::default(),
            summary: "done".to_owned(),
            freshness: state.diagnostic_publish_freshness().commit(),
        },
        progress_token: "current".to_owned(),
    });

    assert_eq!(state.qihe.active_progress_token, None);
}

#[test]
fn cancelled_qihe_finished_result_does_not_commit_diagnostics() {
    let (_root, mut state) = new_test_state("cancelled-qihe-finished-result");
    let file_id = FileId(0);
    let diagnostic = Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 1)),
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("qihe".to_owned()),
        message: "cancelled result".to_owned(),
        ..Diagnostic::default()
    };
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    state.qihe.run_generation = QiheRunId::new(1);
    state.qihe.active_progress_token = Some("current".to_owned());
    state.qihe.active_cancel_token = Some(cancellation);

    state.handle_qihe_task(QiheTask::Finished {
        run_id: QiheRunId::new(1),
        update: QiheUpdate {
            by_file: rustc_hash::FxHashMap::from_iter([(file_id, vec![diagnostic])]),
            summary: "done".to_owned(),
            freshness: state.diagnostic_publish_freshness().commit(),
        },
        progress_token: "current".to_owned(),
    });

    assert!(state.qihe.diagnostics.lock().get(&file_id).is_none());
    assert_eq!(state.qihe.active_progress_token, None);
    assert!(state.qihe.active_cancel_token.is_none());
}

#[test]
fn work_done_progress_cancel_cancels_active_qihe_run() {
    let (_root, mut state) = new_test_state("cancel-active-qihe-run");
    let uri = lsp_types::Url::parse("file:///workspace/top.sv").unwrap();
    let progress_token = qihe_progress_token(QiheRunId::new(7), &uri);
    let token = CancellationToken::new();
    state.qihe.active_progress_token = Some(progress_token.clone());
    state.qihe.active_cancel_token = Some(token.clone());

    state.cancel_work_done_progress(lsp_types::WorkDoneProgressCancelParams {
        token: NumberOrString::String(progress_token),
    });

    assert!(token.is_cancelled());
}

#[test]
fn work_done_progress_cancel_ignores_stale_qihe_run_token() {
    let (_root, mut state) = new_test_state("cancel-stale-qihe-run");
    let uri = lsp_types::Url::parse("file:///workspace/top.sv").unwrap();
    let active_token = qihe_progress_token(QiheRunId::new(8), &uri);
    let stale_token = qihe_progress_token(QiheRunId::new(7), &uri);
    let cancellation = CancellationToken::new();
    state.qihe.active_progress_token = Some(active_token);
    state.qihe.active_cancel_token = Some(cancellation.clone());

    state.cancel_work_done_progress(lsp_types::WorkDoneProgressCancelParams {
        token: NumberOrString::String(stale_token),
    });

    assert!(!cancellation.is_cancelled());
}

#[test]
fn qihe_diagnostics_are_scoped_to_diagnostic_commit_freshness() {
    let root = TestDir::new("qihe-diagnostic-freshness");
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        lsp_types::ClientCapabilities::default(),
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, _client) = lsp_server::Connection::memory();
    let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
    let file_id = FileId(0);
    let diagnostic = Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 1)),
        severity: Some(DiagnosticSeverity::WARNING),
        source: Some("qihe".to_owned()),
        message: "current".to_owned(),
        ..Diagnostic::default()
    };
    let freshness = state.diagnostic_publish_freshness().commit();
    state.qihe.diagnostics.lock().insert(
        file_id,
        QiheDiagnosticState { freshness, generation: 1, diagnostics: vec![diagnostic.clone()] },
    );

    assert_eq!(state.make_snapshot().qihe_diagnostics(file_id), vec![diagnostic]);

    state.diagnostics.diagnostics_revision += 1;
    let snapshot = state.make_snapshot();
    assert!(snapshot.qihe_diagnostics(file_id).is_empty());
}

#[test]
fn qihe_result_with_stale_diagnostic_freshness_does_not_commit() {
    let root = TestDir::new("stale-qihe-freshness");
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        lsp_types::ClientCapabilities::default(),
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, _client) = lsp_server::Connection::memory();
    let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
    state.qihe.run_generation = QiheRunId::new(1);
    state.qihe.active_progress_token = Some("current".to_owned());
    let freshness = state.diagnostic_publish_freshness().commit();
    state.diagnostics.diagnostics_revision += 1;

    state.handle_qihe_task(QiheTask::Finished {
        run_id: QiheRunId::new(1),
        update: QiheUpdate {
            by_file: rustc_hash::FxHashMap::from_iter([(
                FileId(0),
                vec![Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                    severity: Some(DiagnosticSeverity::WARNING),
                    source: Some("qihe".to_owned()),
                    message: "stale".to_owned(),
                    ..Diagnostic::default()
                }],
            )]),
            summary: "old workspace".to_owned(),
            freshness,
        },
        progress_token: "current".to_owned(),
    });

    assert!(state.qihe.diagnostics.lock().is_empty());
    assert_eq!(state.qihe.active_progress_token, None);
}

#[test]
fn qihe_diagnostics_use_pull_refresh_for_pull_capable_clients() {
    let root = TestDir::new("qihe-pull-diagnostics");
    let caps = lsp_types::ClientCapabilities {
        text_document: Some(TextDocumentClientCapabilities {
            diagnostic: Some(DiagnosticClientCapabilities::default()),
            ..Default::default()
        }),
        workspace: Some(WorkspaceClientCapabilities {
            diagnostic: Some(DiagnosticWorkspaceClientCapabilities { refresh_support: Some(true) }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let config = config::Config::new(
        Opt {
            process_name: "vide-test".to_string(),
            log: "error".to_string(),
            log_filename: None,
            profile_trace: None,
        },
        root.path().to_path_buf(),
        caps,
        vec![root.path().to_path_buf()],
        I18n::default(),
        UserConfig::default(),
        Vec::new(),
    );
    let (server, client) = lsp_server::Connection::memory();
    let mut state = GlobalState::new(server.sender, config, TraceValue::Off);

    state.publish_qihe_diagnostics(rustc_hash::FxHashSet::from_iter([FileId(0)]));

    let message = client
        .receiver
        .recv_timeout(std::time::Duration::from_millis(50))
        .expect("expected workspace diagnostic refresh request");
    match message {
        lsp_server::Message::Request(request) => {
            assert_eq!(request.method, lsp_types::request::WorkspaceDiagnosticRefresh::METHOD);
        }
        other => panic!("expected workspace diagnostic refresh request, got {other:?}"),
    }
    assert!(
        client.receiver.recv_timeout(std::time::Duration::from_millis(50)).is_err(),
        "pull-capable clients should not receive forced Qihe publishDiagnostics"
    );
}

#[test]
fn split_compile_args_preserves_forwarded_slang_args() {
    let args = ["--mode", "sv", "--", "-I", "include"].map(ToOwned::to_owned).to_vec();

    let (qihe_args, slang_args) = split_compile_args(&args);

    assert_eq!(qihe_args, ["--mode", "sv"]);
    assert_eq!(slang_args, ["-I", "include"]);
}

#[test]
fn detects_existing_compile_mode() {
    assert!(has_compile_mode(&["--mode".to_owned(), "sv".to_owned()]));
    assert!(has_compile_mode(&["--mode=sv".to_owned()]));
    assert!(has_compile_mode(&["-m".to_owned(), "sv".to_owned()]));
    assert!(!has_compile_mode(&["--foo".to_owned()]));
}

#[test]
fn project_compile_command_synthesizes_sv_mode_and_slang_args() {
    let config = QiheConfig {
        command: "qihe".to_owned(),
        auto_configure_args_from_manifest: true,
        compile_args: vec!["--flag".to_owned(), "--".to_owned(), "--lint".to_owned()],
        run_args: Vec::new(),
    };
    let input = QiheCompileInput {
        files: vec![PathBuf::from("/repo/rtl/a.sv"), PathBuf::from("/repo/rtl/b.sv")],
        manifest_slang_args: vec![
            "--top".to_owned(),
            "top".to_owned(),
            "-I".to_owned(),
            "/repo/include".to_owned(),
            "-DDEBUG".to_owned(),
        ],
        source: QiheCompileInputSource::Manifest(ProjectManifestFileName::Primary),
    };
    let mut command = Command::new("qihe");

    prepare_qihe_compile_command(
        &mut command,
        &config,
        &input,
        PathBuf::from("/tmp/in.qh").as_path(),
    );

    let args = command_args(&command);
    assert_eq!(
        args,
        [
            "--flag",
            "--mode",
            "sv",
            "/repo/rtl/a.sv",
            "/repo/rtl/b.sv",
            "-o",
            "/tmp/in.qh",
            "--",
            "--lint",
            "--top",
            "top",
            "-I",
            "/repo/include",
            "-DDEBUG",
        ]
    );
}

#[test]
fn project_compile_command_can_disable_manifest_args() {
    let config = QiheConfig {
        command: "qihe".to_owned(),
        auto_configure_args_from_manifest: false,
        compile_args: vec![
            "--mode".to_owned(),
            "custom".to_owned(),
            "--".to_owned(),
            "--lint".to_owned(),
        ],
        run_args: Vec::new(),
    };
    let input = QiheCompileInput {
        files: vec![PathBuf::from("/repo/rtl/a.sv"), PathBuf::from("/repo/rtl/b.sv")],
        manifest_slang_args: vec![
            "--top".to_owned(),
            "top".to_owned(),
            "-I".to_owned(),
            "/repo/include".to_owned(),
            "-DDEBUG".to_owned(),
        ],
        source: QiheCompileInputSource::Manifest(ProjectManifestFileName::Primary),
    };
    let mut command = Command::new("qihe");

    prepare_qihe_compile_command(
        &mut command,
        &config,
        &input,
        PathBuf::from("/tmp/in.qh").as_path(),
    );

    assert_eq!(
        command_args(&command),
        [
            "--mode",
            "custom",
            "/repo/rtl/a.sv",
            "/repo/rtl/b.sv",
            "-o",
            "/tmp/in.qh",
            "--",
            "--lint",
        ]
    );
}

#[test]
fn single_file_compile_command_does_not_force_sv_mode() {
    let config = QiheConfig {
        command: "qihe".to_owned(),
        auto_configure_args_from_manifest: true,
        compile_args: Vec::new(),
        run_args: Vec::new(),
    };
    let input = QiheCompileInput {
        files: vec![PathBuf::from("/repo/top.sv")],
        manifest_slang_args: Vec::new(),
        source: QiheCompileInputSource::SingleFile,
    };
    let mut command = Command::new("qihe");

    prepare_qihe_compile_command(
        &mut command,
        &config,
        &input,
        PathBuf::from("/tmp/in.qh").as_path(),
    );

    assert_eq!(command_args(&command), ["/repo/top.sv", "-o", "/tmp/in.qh"]);
}

#[test]
fn run_plan_falls_back_to_temp_storage_without_options_file() {
    let root = TestDir::new("qihe-run-paths-no-options");
    let active_path = root.path().join("top.sv");
    fs::write(&active_path, "module top; endmodule\n").unwrap();
    let run_plan = resolve_qihe_run_plan(active_path.as_path(), root.path().as_ref(), &[]).unwrap();

    assert!(run_plan.ir_path.starts_with(std::env::temp_dir()));
    assert!(run_plan.storage_root.starts_with(std::env::temp_dir()));
    assert!(run_plan.options_path.is_none());
    assert!(run_plan.append_storage_root_arg);
}

#[test]
fn run_plan_uses_storage_root_from_qihe_options() {
    let root = TestDir::new("qihe-run-paths-options-storage");
    let active_path = root.path().join("top.sv");
    fs::write(&active_path, "module top; endmodule\n").unwrap();
    fs::write(root.path().join("qihe-options.toml"), "[storage]\nroot = \"artifacts/qihe\"\n")
        .unwrap();
    let run_plan = resolve_qihe_run_plan(active_path.as_path(), root.path().as_ref(), &[]).unwrap();

    assert_eq!(run_plan.storage_root, PathBuf::from(root.path().join("artifacts/qihe")));
    assert_eq!(run_plan.options_path, Some(PathBuf::from(root.path().join("qihe-options.toml"))));
    assert!(run_plan.ir_path.parent().is_some_and(|path| path.is_dir()));
    assert!(run_plan.append_options_arg);
    assert!(!run_plan.append_storage_root_arg);
}

#[test]
fn run_plan_falls_back_when_qihe_options_has_no_storage_root() {
    let root = TestDir::new("qihe-run-paths-options-no-storage");
    let active_path = root.path().join("top.sv");
    fs::write(&active_path, "module top; endmodule\n").unwrap();
    fs::write(root.path().join("qihe-options.toml"), "[storage]\n").unwrap();
    let run_plan = resolve_qihe_run_plan(active_path.as_path(), root.path().as_ref(), &[]).unwrap();

    assert!(run_plan.storage_root.starts_with(std::env::temp_dir()));
    assert_eq!(run_plan.options_path, Some(PathBuf::from(root.path().join("qihe-options.toml"))));
    assert!(run_plan.append_options_arg);
    assert!(run_plan.append_storage_root_arg);
}

#[test]
fn run_plan_prefers_explicit_storage_root_from_run_args() {
    let root = TestDir::new("qihe-run-plan-explicit-storage-root");
    let active_path = root.path().join("top.sv");
    fs::write(&active_path, "module top; endmodule\n").unwrap();
    fs::write(root.path().join("qihe-options.toml"), "[storage]\nroot = \"artifacts/qihe\"\n")
        .unwrap();
    let run_args = vec![
        "-c".to_owned(),
        "cfg.dump=true".to_owned(),
        "-c".to_owned(),
        "storage.root=./my-storage/".to_owned(),
    ];

    let run_plan =
        resolve_qihe_run_plan(active_path.as_path(), root.path().as_ref(), &run_args).unwrap();

    assert_eq!(run_plan.storage_root, PathBuf::from(root.path().join("my-storage")));
    assert_eq!(run_plan.options_path, Some(PathBuf::from(root.path().join("qihe-options.toml"))));
    assert!(run_plan.append_options_arg);
    assert!(!run_plan.append_storage_root_arg);
}

#[test]
fn run_command_uses_options_file_without_overriding_storage_root() {
    let config = QiheConfig {
        command: "qihe".to_owned(),
        auto_configure_args_from_manifest: true,
        compile_args: Vec::new(),
        run_args: vec!["-g".to_owned(), "std".to_owned()],
    };
    let run_plan = QiheRunPlan {
        ir_path: PathBuf::from("/tmp/in.qh"),
        options_path: Some(PathBuf::from("/repo/qihe-options.toml")),
        storage_root: PathBuf::from("/repo/artifacts/qihe"),
        append_options_arg: true,
        append_storage_root_arg: false,
    };
    let mut command = Command::new("qihe");

    prepare_qihe_run_command(&mut command, &config, &run_plan);

    assert_eq!(
        command_args(&command),
        ["-g", "std", "--options", QIHE_OPTIONS_RUN_PATH, "-i", "/tmp/in.qh"]
    );
}

#[test]
fn run_command_falls_back_to_temp_storage_override() {
    let config = QiheConfig {
        command: "qihe".to_owned(),
        auto_configure_args_from_manifest: true,
        compile_args: Vec::new(),
        run_args: vec!["-g".to_owned(), "std".to_owned()],
    };
    let run_plan = QiheRunPlan {
        ir_path: PathBuf::from("/tmp/in.qh"),
        options_path: None,
        storage_root: PathBuf::from("/tmp/storage"),
        append_options_arg: false,
        append_storage_root_arg: true,
    };
    let mut command = Command::new("qihe");

    prepare_qihe_run_command(&mut command, &config, &run_plan);

    assert_eq!(
        command_args(&command),
        ["-g", "std", "-i", "/tmp/in.qh", "-c", "storage.root=/tmp/storage"]
    );
}

#[test]
fn empty_project_plan_falls_back_to_single_file_input() {
    let active_path = if cfg!(windows) {
        AbsPathBuf::assert("C:/repo/top.sv".into())
    } else {
        AbsPathBuf::assert("/repo/top.sv".into())
    };
    let plan = CompilationPlan::default();

    let input = qihe_compile_input_from_plan(
        &plan,
        Vec::new(),
        active_path.as_ref(),
        ProjectManifestFileName::Primary,
    );

    assert_eq!(
        input,
        QiheCompileInput {
            files: vec![active_path.into()],
            manifest_slang_args: Vec::new(),
            source: QiheCompileInputSource::SingleFile,
        }
    );
}

fn command_args(command: &Command) -> Vec<&str> {
    command.get_args().map(OsStr::to_str).collect::<Option<Vec<_>>>().expect("utf-8 command args")
}
