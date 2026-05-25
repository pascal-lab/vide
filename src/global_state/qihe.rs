use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use base_db::compilation_plan::CompilationPlan;
use lsp_types::{Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, NumberOrString};
use project_model::project_manifest::{ProjectManifest, ProjectManifestFileName};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use span::FileRange;
use utils::{
    line_index::{LineCol, TextRange, TextSize},
    path_identity::PathIdentityIndex,
    paths::{AbsPath, AbsPathBuf},
    thread::ThreadIntent,
};
use vfs::FileId;

use super::{
    GlobalState, QiheDiagnosticState,
    diagnostics::DiagnosticCommitFreshness,
    main_loop::{PublishDiagnosticsBatch, PublishDiagnosticsTask, QiheTask},
    respond::Progress,
    snapshot::GlobalStateSnapshot,
};
use crate::{
    config::user_config::QiheConfig,
    global_state::main_loop::Task,
    i18n::{I18n, keys},
    lsp_ext::{
        ext::{
            QiheLogNotification, QiheLogParams, QiheStatusNotification, QiheStatusParams,
            RunQiheAnalysisParams,
        },
        from_proto, to_proto,
    },
};

#[derive(Debug)]
pub(crate) struct QiheUpdate {
    by_file: FxHashMap<FileId, Vec<Diagnostic>>,
    summary: String,
    freshness: DiagnosticCommitFreshness,
}

const QIHE: &str = "qihe";

static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1B\[[0-?]*[ -/]*[@-~]").unwrap());

/// Monotonic identity for a Qihe analysis run.
///
/// The server keeps latest-run semantics for Qihe: logs, diagnostics, and
/// progress completion from older runs are ignored once a newer run starts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub(crate) struct QiheRunId(u64);

impl QiheRunId {
    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[cfg(test)]
    fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone)]
struct QiheLogSink {
    sender: crossbeam_channel::Sender<Task>,
    run_id: QiheRunId,
    token: String,
}

impl QiheLogSink {
    fn new(sender: crossbeam_channel::Sender<Task>, run_id: QiheRunId, token: String) -> Self {
        Self { sender, run_id, token }
    }

    fn log(&self, message: impl Into<String>) {
        let task = Task::Qihe(QiheTask::Log {
            run_id: self.run_id,
            token: self.token.clone(),
            message: message.into(),
        });
        if self.sender.send(task).is_err() {
            tracing::debug!("qihe log dropped because main loop receiver is closed");
        }
    }
}

impl QiheUpdate {
    fn from_json_diagnostics(
        active_file_id: FileId,
        diagnostics: Vec<QiheJsonDiagnostic>,
        converter: &DiagnosticConverter<'_>,
        freshness: DiagnosticCommitFreshness,
    ) -> Result<Self> {
        let total = diagnostics.len();
        let mut by_file = FxHashMap::from_iter([(active_file_id, Vec::new())]);

        for diagnostic in diagnostics {
            let (file_id, diagnostic) = converter.convert(diagnostic).context(
                converter.snapshot.config.i18n.text(keys::QIHE_CONVERT_DIAGNOSTIC_FAILED),
            )?;
            by_file.entry(file_id).or_default().push(diagnostic);
        }

        let summary = converter
            .snapshot
            .config
            .i18n
            .format(keys::QIHE_FINISHED, [("total", total.to_string())]);
        Ok(Self { by_file, summary, freshness })
    }
}

impl GlobalState {
    pub(crate) fn spawn_qihe_analysis(&mut self, params: RunQiheAnalysisParams) {
        self.end_superseded_qihe_progress();
        self.qihe_run_generation = self.qihe_run_generation.next();
        let run_id = self.qihe_run_generation;
        let progress_token = format!("qihe-analysis:{}", params.uri);
        let progress_label = params.uri.path().to_string();
        let snapshot = self.make_snapshot();

        self.qihe_active_progress_token = Some(progress_token.clone());
        self.begin_qihe_progress(&progress_token, progress_label);

        self.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, move |sender| {
            let log_sink = QiheLogSink::new(sender.clone(), run_id, progress_token.clone());
            let task =
                Task::Qihe(run_qihe_task(snapshot, params, run_id, progress_token, log_sink));
            if sender.send(task).is_err() {
                tracing::debug!("qihe result dropped because main loop receiver is closed");
            }
        });
    }

    pub(crate) fn handle_qihe_task(&mut self, task: QiheTask) {
        match task {
            QiheTask::Log { run_id, token, message } => {
                if run_id == self.qihe_run_generation {
                    self.log_qihe_message(token, message);
                }
            }
            QiheTask::Finished { run_id, update, progress_token } => {
                if run_id != self.qihe_run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.qihe_run_generation,
                        "stale qihe result ignored"
                    );
                    return;
                }
                if update.freshness != self.diagnostic_commit_freshness() {
                    tracing::debug!(
                        ?run_id,
                        freshness = ?update.freshness,
                        current = ?self.diagnostic_commit_freshness(),
                        "stale qihe diagnostics ignored"
                    );
                    let message = self.config.i18n.text(keys::QIHE_STALE).to_owned();
                    self.end_current_qihe_progress(progress_token, "end", message.clone(), message);
                    return;
                }
                let summary = update.summary.clone();
                let changed_files = self.replace_qihe_diagnostics(update.by_file);
                self.publish_qihe_diagnostics(changed_files);
                self.end_current_qihe_progress(progress_token, "end", summary.clone(), summary);
            }
            QiheTask::Failed { run_id, message, progress_token } => {
                if run_id != self.qihe_run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.qihe_run_generation,
                        "stale qihe failure ignored"
                    );
                    return;
                }
                self.end_current_qihe_progress(
                    progress_token,
                    "failed",
                    message.clone(),
                    self.config.i18n.text(keys::QIHE_FAILED).to_owned(),
                );
            }
        }
    }

    fn end_superseded_qihe_progress(&mut self) {
        let Some(progress_token) = self.qihe_active_progress_token.take() else {
            return;
        };

        let message = "Superseded by newer Qihe analysis".to_owned();
        self.end_qihe_progress(progress_token, "end", message.clone(), message);
    }

    fn end_current_qihe_progress(
        &mut self,
        progress_token: String,
        state: &str,
        message: String,
        progress_message: String,
    ) {
        if self.qihe_active_progress_token.as_deref() == Some(progress_token.as_str()) {
            self.qihe_active_progress_token = None;
        }
        self.end_qihe_progress(progress_token, state, message, progress_message);
    }

    fn replace_qihe_diagnostics(
        &mut self,
        mut by_file: FxHashMap<FileId, Vec<Diagnostic>>,
    ) -> FxHashSet<FileId> {
        let mut cache = self.qihe_diagnostics.lock();
        let mut changed_files = cache
            .iter()
            .filter_map(|(&file_id, state)| (!state.diagnostics.is_empty()).then_some(file_id))
            .collect::<FxHashSet<_>>();
        changed_files.extend(by_file.keys().copied());

        for file_id in &changed_files {
            let diagnostics = by_file.remove(file_id).unwrap_or_default();
            let generation =
                cache.get(file_id).map_or(1, |state| state.generation.saturating_add(1));
            cache.insert(
                *file_id,
                QiheDiagnosticState {
                    freshness: self.diagnostic_commit_freshness(),
                    generation,
                    diagnostics,
                },
            );
        }

        changed_files
    }

    fn publish_qihe_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        if changed_files.is_empty() {
            return;
        }

        if self.config.cli_pull_diagnostics_support() {
            self.invalidate_diagnostics(
                super::process_changes::DiagnosticInvalidation::FileChanges(changed_files),
            );
            return;
        }

        let snapshot = self.make_snapshot();
        let mut publish_tasks = Vec::with_capacity(changed_files.len());
        let mut touched_file_ids = FxHashSet::default();
        for file_id in changed_files.iter().copied() {
            let targets = match snapshot.diagnostic_publish_targets(file_id) {
                Ok(targets) => targets,
                Err(error) => {
                    tracing::debug!(
                        ?file_id,
                        "skipping qihe diagnostics for file without URI: {error:#}"
                    );
                    continue;
                }
            };
            let diagnostics = match snapshot.lsp_diagnostics(file_id) {
                Ok(diagnostics) => diagnostics,
                Err(error) if error.is::<ide::Cancelled>() => {
                    tracing::debug!(?file_id, "qihe diagnostic publish cancelled");
                    continue;
                }
                Err(error) => {
                    tracing::debug!(?file_id, "qihe diagnostic publish failed: {error:#}");
                    continue;
                }
            };
            touched_file_ids.insert(file_id);

            publish_tasks.extend(
                targets
                    .into_iter()
                    .map(|target| PublishDiagnosticsTask::from_target(target, diagnostics.clone())),
            );
        }
        self.publish_diagnostics_tasks(PublishDiagnosticsBatch::for_touched_files(
            touched_file_ids,
            publish_tasks,
            snapshot.diagnostic_publish_freshness,
        ));
    }

    fn begin_qihe_progress(&mut self, progress_token: &str, label: String) {
        self.send_qihe_status(progress_token, "begin", Some(label.clone()));
        self.report_qihe_progress(Progress::Begin, label, None, progress_token.to_owned());
    }

    fn end_qihe_progress(
        &mut self,
        token: String,
        state: &str,
        message: String,
        progress_message: String,
    ) {
        self.send_qihe_status(&token, state, Some(message.clone()));
        self.log_qihe_message(token.clone(), message);
        self.report_qihe_progress(Progress::End, progress_message, Some(1.0), token);
    }

    fn report_qihe_progress(
        &mut self,
        state: Progress,
        message: String,
        fraction: Option<f64>,
        token: String,
    ) {
        self.report_progress(
            self.config.i18n.text(keys::QIHE_PROGRESS_TITLE),
            state,
            Some(message),
            fraction,
            Some(token),
        );
    }

    fn send_qihe_status(&mut self, token: &str, state: &str, message: Option<String>) {
        self.send_notification::<QiheStatusNotification>(QiheStatusParams {
            token: token.to_owned(),
            state: state.to_owned(),
            message,
        });
    }

    fn log_qihe_message(&mut self, token: String, message: String) {
        self.send_notification::<QiheLogNotification>(QiheLogParams { token, message });
    }
}

fn run_qihe_task(
    snapshot: GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
    run_id: QiheRunId,
    progress_token: String,
    log_sink: QiheLogSink,
) -> QiheTask {
    match run_qihe_request(&snapshot, params, &log_sink) {
        Ok(update) => QiheTask::Finished { run_id, update, progress_token },
        Err(err) => QiheTask::Failed { run_id, message: err.to_string(), progress_token },
    }
}

fn run_qihe_request(
    snapshot: &GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
    log_sink: &QiheLogSink,
) -> Result<QiheUpdate> {
    let active_path = from_proto::abs_path(&params.uri)?;
    let active_file_id = snapshot.file_id(&params.uri)?;
    let qihe_config = snapshot.config.qihe();
    let cwd = qihe_working_directory(params.cwd, snapshot.config.root_path.as_path());
    let compile_input = qihe_compile_input(snapshot, active_file_id, active_path.as_path(), &cwd)?;
    if let Some(manifest_file_name) = compile_input.deprecated_manifest_file_name() {
        log_sink.log(format!(
            "Project manifest `{}` is deprecated; rename it to `{}` when possible.",
            manifest_file_name.as_str(),
            ProjectManifestFileName::Primary.as_str()
        ));
    }
    let i18n = snapshot.config.i18n;
    let (ir_path, storage_root) = qihe_run_paths(active_path.as_path())
        .context(i18n.text(keys::QIHE_PREPARE_WORKSPACE_FAILED))?;
    run_qihe_commands(&qihe_config, &cwd, &compile_input, &ir_path, &storage_root, i18n, log_sink)?;

    let diagnostics = load_latest_diagnostics(&storage_root, i18n)?;
    let resolution_base = if compile_input.uses_manifest() {
        cwd.as_path()
    } else {
        active_path
            .as_path()
            .parent()
            .map(AsRef::as_ref)
            .unwrap_or_else(|| active_path.as_path().as_ref())
    };
    let converter =
        DiagnosticConverter { snapshot, default_file_id: active_file_id, resolution_base };
    QiheUpdate::from_json_diagnostics(
        active_file_id,
        diagnostics,
        &converter,
        snapshot.diagnostic_commit_freshness(),
    )
}

fn qihe_working_directory(params_cwd: Option<PathBuf>, root_path: &AbsPath) -> PathBuf {
    params_cwd
        .and_then(|path| dunce::canonicalize(path).ok())
        .unwrap_or_else(|| root_path.to_path_buf().into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QiheCompileInput {
    files: Vec<PathBuf>,
    manifest_slang_args: Vec<String>,
    source: QiheCompileInputSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QiheCompileInputSource {
    SingleFile,
    Manifest(ProjectManifestFileName),
}

impl QiheCompileInput {
    fn uses_manifest(&self) -> bool {
        matches!(self.source, QiheCompileInputSource::Manifest(_))
    }

    fn deprecated_manifest_file_name(&self) -> Option<ProjectManifestFileName> {
        match self.source {
            QiheCompileInputSource::Manifest(file_name) if file_name.is_deprecated() => {
                Some(file_name)
            }
            QiheCompileInputSource::SingleFile | QiheCompileInputSource::Manifest(_) => None,
        }
    }
}

fn qihe_compile_input(
    snapshot: &GlobalStateSnapshot,
    active_file_id: FileId,
    active_path: &AbsPath,
    cwd: &Path,
) -> Result<QiheCompileInput> {
    let Some(manifest_file_name) =
        qihe_project_manifest_file_name(&snapshot.config.project_manifests, cwd)?
    else {
        return Ok(single_file_qihe_compile_input(active_path));
    };

    let plan = snapshot
        .analysis
        .compilation_plan(active_file_id)
        .map_err(|_| qihe_cancelled(snapshot.config.i18n))?;
    let files = plan
        .roots
        .iter()
        .filter_map(|file_id| snapshot.file_path(*file_id).map(PathBuf::from))
        .collect::<Vec<_>>();

    Ok(qihe_compile_input_from_plan(&plan, files, active_path, manifest_file_name))
}

fn qihe_project_manifest_file_name(
    manifests: &[ProjectManifest],
    cwd: &Path,
) -> Result<Option<ProjectManifestFileName>> {
    let cwd = AbsPathBuf::try_from(cwd.to_path_buf()).map_err(|path| {
        anyhow!("Qihe working directory must be an absolute UTF-8 path: {}", path.display())
    })?;

    let mut manifest_roots = PathIdentityIndex::default();
    for manifest in manifests {
        let Some(file_name) = manifest.toml_file_name() else {
            continue;
        };
        let Some(root) = project_manifest_workspace_root(manifest) else {
            continue;
        };
        manifest_roots.insert_path(root, file_name);
    }

    Ok(manifest_roots.get_path(cwd.as_path()))
}

fn project_manifest_workspace_root(manifest: &ProjectManifest) -> Option<&AbsPath> {
    match manifest {
        ProjectManifest::Toml(path) => path.parent(),
        ProjectManifest::UnconfiguredRoot(path) => Some(path.as_path()),
    }
}

fn single_file_qihe_compile_input(active_path: &AbsPath) -> QiheCompileInput {
    QiheCompileInput {
        files: vec![active_path.to_path_buf().into()],
        manifest_slang_args: Vec::new(),
        source: QiheCompileInputSource::SingleFile,
    }
}

fn qihe_compile_input_from_plan(
    plan: &CompilationPlan,
    mut files: Vec<PathBuf>,
    active_path: &AbsPath,
    manifest_file_name: ProjectManifestFileName,
) -> QiheCompileInput {
    files.sort();
    files.dedup();

    if files.is_empty() {
        return single_file_qihe_compile_input(active_path);
    }

    let mut slang_args = Vec::new();
    for top_module in &plan.top_modules {
        slang_args.push("--top".to_owned());
        slang_args.push(top_module.clone());
    }
    for include_dir in &plan.include_dirs {
        slang_args.push("-I".to_owned());
        slang_args.push(include_dir.to_string());
    }
    for define in &plan.predefines {
        slang_args.push(format!("-D{define}"));
    }

    QiheCompileInput {
        files,
        manifest_slang_args: slang_args,
        source: QiheCompileInputSource::Manifest(manifest_file_name),
    }
}

fn qihe_run_paths(active_path: &AbsPath) -> Result<(PathBuf, PathBuf)> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let workspace = std::env::temp_dir()
        .join(QIHE)
        .join(format!("{}-{millis}", active_path.file_stem().unwrap_or("input")));
    let storage_root = workspace.join("storage");
    fs::create_dir_all(&storage_root)?;
    Ok((workspace.join("input.qh"), storage_root))
}

fn run_qihe_commands(
    qihe_config: &QiheConfig,
    cwd: &Path,
    compile_input: &QiheCompileInput,
    ir_path: &Path,
    storage_root: &Path,
    i18n: I18n,
    log_sink: &QiheLogSink,
) -> Result<()> {
    let mut command = qihe_command(qihe_config, cwd, "compile");
    prepare_qihe_compile_command(&mut command, qihe_config, compile_input, ir_path);
    run_command(&mut command, "qihe compile", i18n, log_sink)?;

    let mut command = qihe_command(qihe_config, cwd, "run");
    run_command(
        command
            .args(&qihe_config.run_args)
            .arg("-i")
            .arg(ir_path)
            .arg("-c")
            .arg(format!("storage.root={}", storage_root.display())),
        "qihe run",
        i18n,
        log_sink,
    )
}

fn prepare_qihe_compile_command(
    command: &mut Command,
    qihe_config: &QiheConfig,
    compile_input: &QiheCompileInput,
    ir_path: &Path,
) {
    let (qihe_args, user_slang_args) = split_compile_args(&qihe_config.compile_args);
    command.args(&qihe_args);
    let auto_configure_manifest_args =
        qihe_config.auto_configure_args_from_manifest && compile_input.uses_manifest();
    if auto_configure_manifest_args && !has_compile_mode(&qihe_args) {
        command.args(["--mode", "sv"]);
    }
    command.args(&compile_input.files).arg("-o").arg(ir_path);

    let manifest_slang_args = if auto_configure_manifest_args {
        compile_input.manifest_slang_args.as_slice()
    } else {
        &[]
    };
    let has_slang_args = !user_slang_args.is_empty() || !manifest_slang_args.is_empty();
    if has_slang_args {
        command.arg("--").args(&user_slang_args).args(manifest_slang_args);
    }
}

fn split_compile_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    let Some(separator) = args.iter().position(|arg| arg == "--") else {
        return (args.to_vec(), Vec::new());
    };
    (args[..separator].to_vec(), args[separator + 1..].to_vec())
}

fn has_compile_mode(args: &[String]) -> bool {
    args.iter().enumerate().any(|(idx, arg)| {
        arg == "--mode"
            || arg.starts_with("--mode=")
            || (arg == "-m" && args.get(idx + 1).is_some())
    })
}

fn qihe_command(qihe_config: &QiheConfig, cwd: &Path, subcommand: &str) -> Command {
    let mut command = Command::new(&qihe_config.command);
    command.current_dir(cwd).arg(subcommand);
    command
}

fn run_command(
    command: &mut Command,
    label: &str,
    i18n: I18n,
    log_sink: &QiheLogSink,
) -> Result<()> {
    let command_line = command_line(command);
    log_sink.log(format!("{label} command:\n{command_line}"));
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().with_context(|| {
        i18n.format(
            keys::QIHE_COMMAND_FAILED_TO_START,
            [("label", label.to_owned()), ("command_line", command_line.clone())],
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .map(|stdout| stream_command_output(stdout, label.to_owned(), "stdout", log_sink.clone()));
    let stderr = child
        .stderr
        .take()
        .map(|stderr| stream_command_output(stderr, label.to_owned(), "stderr", log_sink.clone()));

    let status = child.wait()?;
    let stdout = join_command_output(stdout);
    let stderr = join_command_output(stderr);
    log_sink.log(format!("{label} finished with status {status}"));

    if status.success() {
        return Ok(());
    }

    bail!(
        "{}",
        i18n.format(
            keys::QIHE_COMMAND_FAILED,
            [
                ("label", label.to_owned()),
                ("status", status.to_string()),
                ("command_line", command_line),
                ("stdout", stdout.trim().to_owned()),
                ("stderr", stderr.trim().to_owned()),
            ],
        )
    );
}

fn stream_command_output<R: Read + Send + 'static>(
    stream: R,
    label: String,
    stream_name: &'static str,
    log_sink: QiheLogSink,
) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut output = String::new();
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::new();

        loop {
            bytes.clear();
            let read = match reader.read_until(b'\n', &mut bytes) {
                Ok(read) => read,
                Err(error) => {
                    log_sink.log(format!("{label} {stream_name} read failed: {error}"));
                    break;
                }
            };
            if read == 0 {
                break;
            }

            let chunk = strip_ansi(String::from_utf8_lossy(&bytes).as_ref());
            output.push_str(&chunk);
            log_command_output_line(&label, stream_name, &chunk, &log_sink);
        }

        output
    })
}

fn log_command_output_line(label: &str, stream_name: &str, output: &str, log_sink: &QiheLogSink) {
    let text = output.trim_end_matches(&['\r', '\n'][..]);
    log_sink.log(format!("{label} {stream_name}: {text}"));
}

fn join_command_output(handle: Option<JoinHandle<String>>) -> String {
    let Some(handle) = handle else {
        return String::new();
    };
    handle.join().unwrap_or_default()
}

fn command_line(command: &Command) -> String {
    let mut parts = Vec::new();
    if let Some(cwd) = command.get_current_dir() {
        parts.push(format!("cwd={}", quote_command_arg(cwd.as_os_str())));
    }
    parts.push(quote_command_arg(command.get_program()));
    parts.extend(command.get_args().map(quote_command_arg));
    parts.join(" ")
}

fn quote_command_arg(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if !text.is_empty()
        && text.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b':' | b'.' | b'_' | b'-' | b'=' | b'+')
        })
    {
        return text.into_owned();
    }

    format!("{text:?}")
}

fn strip_ansi(text: &str) -> String {
    ANSI_ESCAPE_RE.replace_all(text, "").into_owned()
}

fn load_latest_diagnostics(storage_root: &Path, i18n: I18n) -> Result<Vec<QiheJsonDiagnostic>> {
    let diagnostics_dir = storage_root.join("diagnostics");
    let Some(latest) = latest_diagnostic_path(&diagnostics_dir, i18n)? else {
        return Ok(Vec::new());
    };

    let text = fs::read_to_string(&latest).with_context(|| {
        i18n.format(keys::QIHE_READ_DIAGNOSTICS_FAILED, [("path", latest.display().to_string())])
    })?;
    serde_json::from_str(&text).with_context(|| {
        i18n.format(keys::QIHE_PARSE_DIAGNOSTICS_FAILED, [("path", latest.display().to_string())])
    })
}

fn latest_diagnostic_path(diagnostics_dir: &Path, i18n: I18n) -> Result<Option<PathBuf>> {
    if !diagnostics_dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<((SystemTime, PathBuf), PathBuf)> = None;
    for entry in fs::read_dir(diagnostics_dir).with_context(|| {
        i18n.format(
            keys::QIHE_READ_DIAGNOSTICS_DIR_FAILED,
            [("path", diagnostics_dir.display().to_string())],
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let modified =
                entry.metadata().and_then(|metadata| metadata.modified()).unwrap_or(UNIX_EPOCH);
            latest = latest.max(Some(((modified, path.clone()), path)));
        }
    }

    Ok(latest.map(|(_, path)| path))
}

struct DiagnosticConverter<'a> {
    snapshot: &'a GlobalStateSnapshot,
    default_file_id: FileId,
    resolution_base: &'a Path,
}

type SourceRange = (FileId, TextRange);

impl<'a> DiagnosticConverter<'a> {
    fn convert(&self, diagnostic: QiheJsonDiagnostic) -> Result<(FileId, Diagnostic)> {
        let QiheJsonDiagnostic { severity, analysis_class, element, message, support_info } =
            diagnostic;

        let mut related_info = Vec::new();
        let mut extra_support_lines = Vec::new();

        for info in &support_info {
            if let Some((file_id, range)) = self.location_from_element(&info.element)? {
                related_info.push(DiagnosticRelatedInformation {
                    location: to_proto::location(self.snapshot, FileRange { file_id, range })
                        .map_err(|_| qihe_cancelled(self.snapshot.config.i18n))?,
                    message: info.message.clone(),
                });
            } else {
                extra_support_lines.push(format!("{} ({})", info.message, info.element));
            }
        }

        let (file_id, range, location_unknown) =
            match self.primary_location(&element, &support_info)? {
                Some((file_id, range)) => (file_id, range, false),
                None => (self.default_file_id, TextRange::empty(TextSize::new(0)), true),
            };

        let message = diagnostic_message(
            self.snapshot.config.i18n,
            message,
            &element,
            location_unknown,
            extra_support_lines,
        );
        let line_info = self
            .snapshot
            .line_info(file_id)
            .map_err(|_| qihe_cancelled(self.snapshot.config.i18n))?;
        let range = to_proto::range(&line_info, range);
        let related_info = (!related_info.is_empty()).then_some(related_info);

        Ok((
            file_id,
            Diagnostic::new(
                range,
                Some(map_severity(&severity)),
                Some(NumberOrString::String(analysis_code(&analysis_class))),
                Some(QIHE.to_owned()),
                message,
                related_info,
                None,
            ),
        ))
    }

    fn primary_location(
        &self,
        element: &str,
        support_info: &[QiheJsonSupportInfo],
    ) -> Result<Option<SourceRange>> {
        std::iter::once(element)
            .chain(support_info.iter().map(|info| info.element.as_str()))
            .find_map(|element| self.location_from_element(element).transpose())
            .transpose()
    }

    fn location_from_element(&self, element: &str) -> Result<Option<SourceRange>> {
        parse_source_loc(element).map_or(Ok(None), |location| self.location_from_source(location))
    }

    fn location_from_source(&self, location: SourceLocation) -> Result<Option<SourceRange>> {
        let file_id = location.file_name.as_deref().map_or(Some(self.default_file_id), |name| {
            resolve_file_name(self.resolution_base, name)
                .and_then(|path| self.snapshot.file_id_for_path(path.as_ref()))
        });
        let Some(file_id) = file_id else { return Ok(None) };

        let line_index = self
            .snapshot
            .analysis
            .line_index(file_id)
            .map_err(|_| qihe_cancelled(self.snapshot.config.i18n))?;
        let line = location.line.saturating_sub(1);
        let col = location.column.saturating_sub(1);
        let Some(offset) = line_index.offset(LineCol { line, col }) else {
            return Ok(None);
        };

        Ok(Some((file_id, TextRange::empty(offset))))
    }
}

fn diagnostic_message(
    i18n: I18n,
    message: String,
    primary_element: &str,
    location_unknown: bool,
    mut extra_support_lines: Vec<String>,
) -> String {
    if location_unknown && !primary_element.is_empty() {
        extra_support_lines.push(
            i18n.format(keys::QIHE_LOCATION, [("primary_element", primary_element.to_owned())]),
        );
    }
    extra_support_lines.insert(0, message);
    extra_support_lines.join("\n")
}

fn analysis_code(analysis_class: &str) -> String {
    analysis_class.rsplit('.').next().filter(|code| !code.is_empty()).unwrap_or("Qihe").to_owned()
}

fn qihe_cancelled(i18n: I18n) -> anyhow::Error {
    anyhow!(i18n.text(keys::QIHE_CANCELLED))
}

fn resolve_file_name(base_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let candidate = Path::new(file_name);
    Some(if candidate.is_absolute() { candidate.to_path_buf() } else { base_dir.join(file_name) })
}

fn map_severity(severity: &str) -> DiagnosticSeverity {
    match severity.trim().to_ascii_uppercase().as_str() {
        "ERROR" => DiagnosticSeverity::ERROR,
        "WARNING" | "WARN" => DiagnosticSeverity::WARNING,
        "INFO" | "INFORMATION" => DiagnosticSeverity::INFORMATION,
        "HINT" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::WARNING,
    }
}

fn parse_source_loc(raw: &str) -> Option<SourceLocation> {
    let raw = raw.trim();
    if matches!(raw.as_bytes().first(), None | Some(b'@' | b'#')) {
        return None;
    }

    let (head, column) = raw.rsplit_once(':')?;
    let column = column.parse().ok()?;
    let Some((file_name, line)) = head.rsplit_once(':') else {
        let line = head.parse().ok()?;
        return Some(SourceLocation { file_name: None, line, column });
    };
    let line = line.parse().ok()?;

    (!file_name.is_empty()).then(|| SourceLocation {
        file_name: Some(file_name.to_string()),
        line,
        column,
    })
}

#[derive(Debug, Clone)]
struct SourceLocation {
    file_name: Option<String>,
    line: u32,
    column: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QiheJsonDiagnostic {
    severity: String,
    analysis_class: String,
    element: String,
    message: String,
    #[serde(default)]
    support_info: Vec<QiheJsonSupportInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QiheJsonSupportInfo {
    element: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, io::Cursor, path::PathBuf, process::Command};

    use base_db::compilation_plan::CompilationPlan;
    use crossbeam_channel::unbounded;
    use lsp_types::{
        Diagnostic, DiagnosticClientCapabilities, DiagnosticSeverity,
        DiagnosticWorkspaceClientCapabilities, Position, Range, TextDocumentClientCapabilities,
        TraceValue, WorkspaceClientCapabilities, request::Request,
    };
    use project_model::project_manifest::{
        LEGACY_MANIFEST_FILE_NAME, ProjectManifest, ProjectManifestFileName,
    };
    use utils::{paths::AbsPathBuf, test_support::TestDir};
    use vfs::FileId;

    use super::{
        QiheCompileInput, QiheCompileInputSource, QiheLogSink, QiheRunId, QiheUpdate, command_line,
        has_compile_mode, join_command_output, parse_source_loc, prepare_qihe_compile_command,
        qihe_compile_input_from_plan, qihe_project_manifest_file_name, qihe_working_directory,
        split_compile_args, stream_command_output, strip_ansi,
    };
    use crate::{
        Opt,
        config::{
            self,
            user_config::{QiheConfig, UserConfig},
        },
        global_state::{
            GlobalState, QiheDiagnosticState,
            main_loop::{QiheTask, Task},
        },
        i18n::I18n,
    };

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
        assert_eq!(messages, ["qihe run stdout: first", "qihe run stdout: second"]);
    }

    #[test]
    fn stale_qihe_result_does_not_replace_current_diagnostics() {
        let root = TestDir::new("stale-qihe-result");
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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
        state.qihe_run_generation = QiheRunId::new(2);
        let file_id = FileId(0);
        let current = Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity: Some(DiagnosticSeverity::WARNING),
            source: Some("qihe".to_owned()),
            message: "current".to_owned(),
            ..Diagnostic::default()
        };
        let stale = Diagnostic { message: "stale".to_owned(), ..current.clone() };
        let freshness = state.diagnostic_commit_freshness();
        state.qihe_diagnostics.lock().insert(
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

        let stored = state.qihe_diagnostics.lock().get(&file_id).unwrap().diagnostics.clone();
        assert_eq!(stored, vec![current]);
    }

    #[test]
    fn current_qihe_result_closes_active_progress() {
        let root = TestDir::new("current-qihe-progress");
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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
        state.qihe_run_generation = QiheRunId::new(1);
        state.qihe_active_progress_token = Some("current".to_owned());

        state.handle_qihe_task(QiheTask::Finished {
            run_id: QiheRunId::new(1),
            update: QiheUpdate {
                by_file: rustc_hash::FxHashMap::default(),
                summary: "done".to_owned(),
                freshness: state.diagnostic_commit_freshness(),
            },
            progress_token: "current".to_owned(),
        });

        assert_eq!(state.qihe_active_progress_token, None);
    }

    #[test]
    fn qihe_diagnostics_are_scoped_to_diagnostic_commit_freshness() {
        let root = TestDir::new("qihe-diagnostic-freshness");
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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
        let freshness = state.diagnostic_commit_freshness();
        state.qihe_diagnostics.lock().insert(
            file_id,
            QiheDiagnosticState { freshness, generation: 1, diagnostics: vec![diagnostic.clone()] },
        );

        assert_eq!(state.make_snapshot().qihe_diagnostics(file_id), vec![diagnostic]);

        state.diagnostics_revision += 1;
        let snapshot = state.make_snapshot();
        assert!(snapshot.qihe_diagnostics(file_id).is_empty());
    }

    #[test]
    fn qihe_result_with_stale_diagnostic_freshness_does_not_commit() {
        let root = TestDir::new("stale-qihe-freshness");
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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
        state.qihe_run_generation = QiheRunId::new(1);
        state.qihe_active_progress_token = Some("current".to_owned());
        let freshness = state.diagnostic_commit_freshness();
        state.diagnostics_revision += 1;

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

        assert!(state.qihe_diagnostics.lock().is_empty());
        assert_eq!(state.qihe_active_progress_token, None);
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
                diagnostic: Some(DiagnosticWorkspaceClientCapabilities {
                    refresh_support: Some(true),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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

    #[test]
    fn legacy_manifest_selects_project_qihe_route() {
        let root = TestDir::new("qihe-legacy-manifest-route");
        root.write(LEGACY_MANIFEST_FILE_NAME, "sources = [\"rtl/**\"]\n");
        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();

        let manifest_file_name =
            qihe_project_manifest_file_name(&[manifest], root.path().as_ref()).unwrap();

        assert_eq!(manifest_file_name, Some(ProjectManifestFileName::Legacy));
    }

    #[test]
    fn legacy_manifest_route_matches_canonicalized_cwd_alias() {
        let root = TestDir::new("qihe-legacy-manifest-canonical-cwd");
        root.write(LEGACY_MANIFEST_FILE_NAME, "sources = [\"rtl/**\"]\n");
        let root_path: &std::path::Path = root.path().as_ref();
        let alias_root = AbsPathBuf::assert_utf8(root_path.join("."));
        let manifest = ProjectManifest::from_path(&alias_root).unwrap();
        let canonical_cwd = dunce::canonicalize(root_path).unwrap();

        let manifest_file_name =
            qihe_project_manifest_file_name(&[manifest], &canonical_cwd).unwrap();

        assert_eq!(manifest_file_name, Some(ProjectManifestFileName::Legacy));
    }

    #[test]
    fn legacy_project_compile_input_reports_deprecated_manifest() {
        let active_path = if cfg!(windows) {
            AbsPathBuf::assert("C:/repo/top.sv".into())
        } else {
            AbsPathBuf::assert("/repo/top.sv".into())
        };
        let plan = CompilationPlan {
            roots: vec![FileId(0)],
            include_dirs: vec![active_path.parent().unwrap().to_path_buf()],
            top_modules: vec!["top".to_owned()],
            predefines: vec!["SYNTHESIS".to_owned()],
            ..CompilationPlan::default()
        };

        let input = qihe_compile_input_from_plan(
            &plan,
            vec![active_path.to_path_buf().into()],
            active_path.as_ref(),
            ProjectManifestFileName::Legacy,
        );

        assert!(input.uses_manifest());
        assert_eq!(input.deprecated_manifest_file_name(), Some(ProjectManifestFileName::Legacy));
    }

    fn command_args(command: &Command) -> Vec<&str> {
        command
            .get_args()
            .map(OsStr::to_str)
            .collect::<Option<Vec<_>>>()
            .expect("utf-8 command args")
    }
}
