use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, LogMessageParams, MessageType,
    NumberOrString, ShowMessageParams,
};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use span::FileRange;
use utils::{
    line_index::{LineCol, TextRange, TextSize},
    paths::AbsPath,
    thread::ThreadIntent,
};
use vfs::FileId;

use super::{
    GlobalState, QiheDiagnosticState,
    main_loop::{PublishDiagnosticsTask, QiheTask},
    respond::Progress,
    snapshot::GlobalStateSnapshot,
};
use crate::{
    config::user_config::QiheConfig,
    global_state::main_loop::Task,
    lsp_ext::{
        ext::{QiheStatusNotification, QiheStatusParams, RunQiheAnalysisParams},
        from_proto, to_proto,
    },
};

#[derive(Debug)]
pub(crate) struct QiheUpdate {
    by_file: FxHashMap<FileId, Vec<Diagnostic>>,
    summary: String,
}

const QIHE_PROGRESS_TITLE: &str = "Running Qihe Analysis";
const QIHE: &str = "qihe";

static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1B\[[0-?]*[ -/]*[@-~]").unwrap());

impl QiheUpdate {
    fn from_json_diagnostics(
        active_file_id: FileId,
        diagnostics: Vec<QiheJsonDiagnostic>,
        converter: &DiagnosticConverter<'_>,
    ) -> Result<Self> {
        let total = diagnostics.len();
        let mut by_file = FxHashMap::from_iter([(active_file_id, Vec::new())]);

        for diagnostic in diagnostics {
            let (file_id, diagnostic) =
                converter.convert(diagnostic).context("failed to convert qihe diagnostic")?;
            by_file.entry(file_id).or_default().push(diagnostic);
        }

        let summary = format!("Qihe analysis finished with {total} diagnostic(s).");
        Ok(Self { by_file, summary })
    }
}

impl GlobalState {
    pub(crate) fn spawn_qihe_analysis(&mut self, params: RunQiheAnalysisParams) {
        let progress_token = format!("qihe-analysis:{}", params.uri);
        let progress_label = params.uri.path().to_string();
        let snapshot = self.make_snapshot();

        self.begin_qihe_progress(&progress_token, progress_label);

        self.task_pool.handle.spawn_and_send(ThreadIntent::Worker, move || {
            Task::Qihe(run_qihe_task(snapshot, params, progress_token))
        });
    }

    pub(crate) fn handle_qihe_task(&mut self, task: QiheTask) {
        match task {
            QiheTask::Finished { update, progress_token } => {
                let summary = update.summary.clone();
                let changed_files = self.replace_qihe_diagnostics(update.by_file);
                self.publish_qihe_diagnostics(changed_files);
                self.end_qihe_progress(
                    progress_token,
                    "end",
                    MessageType::INFO,
                    summary.clone(),
                    summary,
                );
            }
            QiheTask::Failed { message, progress_token } => {
                self.end_qihe_progress(
                    progress_token,
                    "failed",
                    MessageType::ERROR,
                    message.clone(),
                    "Qihe analysis failed".to_owned(),
                );
                self.send_notification::<lsp_types::notification::ShowMessage>(ShowMessageParams {
                    typ: MessageType::ERROR,
                    message,
                });
            }
        }
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
            cache.insert(*file_id, QiheDiagnosticState { generation, diagnostics });
        }

        changed_files
    }

    fn publish_qihe_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        if changed_files.is_empty() {
            return;
        }

        let snapshot = self.make_snapshot();
        let mut publish_tasks = Vec::with_capacity(changed_files.len());
        for file_id in changed_files {
            let uri = match snapshot.url(file_id) {
                Ok(uri) => uri,
                Err(error) => {
                    tracing::debug!(
                        ?file_id,
                        "skipping qihe diagnostics for file without URI: {error:#}"
                    );
                    continue;
                }
            };

            publish_tasks.push(PublishDiagnosticsTask {
                file_id,
                uri,
                version: snapshot.file_version(file_id),
                diagnostics: snapshot.lsp_diagnostics(file_id),
            });
        }
        self.publish_diagnostics_tasks(publish_tasks, true);
    }

    fn begin_qihe_progress(&mut self, progress_token: &str, label: String) {
        self.send_qihe_status(progress_token, "begin", Some(label.clone()));
        self.report_qihe_progress(Progress::Begin, label, None, progress_token.to_owned());
    }

    fn end_qihe_progress(
        &mut self,
        token: String,
        state: &str,
        typ: MessageType,
        message: String,
        progress_message: String,
    ) {
        self.send_qihe_status(&token, state, Some(message.clone()));
        self.log_qihe_message(typ, message);
        self.report_qihe_progress(Progress::End, progress_message, Some(1.0), token);
    }

    fn report_qihe_progress(
        &mut self,
        state: Progress,
        message: String,
        fraction: Option<f64>,
        token: String,
    ) {
        self.report_progress(QIHE_PROGRESS_TITLE, state, Some(message), fraction, Some(token));
    }

    fn send_qihe_status(&mut self, token: &str, state: &str, message: Option<String>) {
        self.send_notification::<QiheStatusNotification>(QiheStatusParams {
            token: token.to_owned(),
            state: state.to_owned(),
            message,
        });
    }

    fn log_qihe_message(&mut self, typ: MessageType, message: String) {
        self.send_notification::<lsp_types::notification::LogMessage>(LogMessageParams {
            typ,
            message,
        });
    }
}

fn run_qihe_task(
    snapshot: GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
    progress_token: String,
) -> QiheTask {
    match run_qihe_request(&snapshot, params) {
        Ok(update) => QiheTask::Finished { update, progress_token },
        Err(err) => QiheTask::Failed { message: err.to_string(), progress_token },
    }
}

fn run_qihe_request(
    snapshot: &GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
) -> Result<QiheUpdate> {
    let active_path = from_proto::abs_path(&params.uri)?;
    let active_file_id = snapshot.file_id(&params.uri)?;
    let qihe_config = snapshot.config.qihe();
    let cwd = params
        .cwd
        .and_then(|path| path.canonicalize().ok())
        .unwrap_or_else(|| snapshot.config.root_path.to_path_buf().into());
    let active_path_buf: PathBuf = active_path.to_path_buf().into();
    let (ir_path, storage_root) =
        qihe_run_paths(active_path.as_path()).context("failed to prepare qihe workspace")?;
    run_qihe_commands(&qihe_config, &cwd, &active_path_buf, &ir_path, &storage_root)?;

    let diagnostics = load_latest_diagnostics(&storage_root)?;
    let converter = DiagnosticConverter {
        snapshot,
        default_file_id: active_file_id,
        default_path: active_path.as_path(),
    };
    QiheUpdate::from_json_diagnostics(active_file_id, diagnostics, &converter)
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
    active_path: &Path,
    ir_path: &Path,
    storage_root: &Path,
) -> Result<()> {
    let mut command = qihe_command(qihe_config, cwd, "compile");
    run_command(
        command.args(&qihe_config.compile_args).arg(active_path).arg("-o").arg(ir_path),
        "qihe compile",
    )?;

    let mut command = qihe_command(qihe_config, cwd, "run");
    run_command(
        command
            .args(&qihe_config.run_args)
            .arg("-i")
            .arg(ir_path)
            .arg("-c")
            .arg(format!("storage.root={}", storage_root.display())),
        "qihe run",
    )
}

fn qihe_command(qihe_config: &QiheConfig, cwd: &Path, subcommand: &str) -> Command {
    let mut command = Command::new(&qihe_config.command);
    command.current_dir(cwd).arg(subcommand);
    command
}

fn run_command(command: &mut Command, label: &str) -> Result<()> {
    let command_line = format!("{command:?}");
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let output =
        command.output().with_context(|| format!("{label} failed to start: {command_line}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stdout = strip_ansi(String::from_utf8_lossy(&output.stdout).as_ref());
    let stderr = strip_ansi(String::from_utf8_lossy(&output.stderr).as_ref());
    bail!(
        "{label} failed with status {}.\ncommand:\n{}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        command_line,
        stdout.trim(),
        stderr.trim()
    );
}

fn strip_ansi(text: &str) -> String {
    ANSI_ESCAPE_RE.replace_all(text, "").into_owned()
}

fn load_latest_diagnostics(storage_root: &Path) -> Result<Vec<QiheJsonDiagnostic>> {
    let diagnostics_dir = storage_root.join("diagnostics");
    let Some(latest) = latest_diagnostic_path(&diagnostics_dir)? else {
        return Ok(Vec::new());
    };

    let text = fs::read_to_string(&latest)
        .with_context(|| format!("failed to read qihe diagnostics at {}", latest.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse qihe diagnostics at {}", latest.display()))
}

fn latest_diagnostic_path(diagnostics_dir: &Path) -> Result<Option<PathBuf>> {
    if !diagnostics_dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<((SystemTime, PathBuf), PathBuf)> = None;
    for entry in fs::read_dir(diagnostics_dir).with_context(|| {
        format!("failed to read qihe diagnostics dir {}", diagnostics_dir.display())
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
    default_path: &'a AbsPath,
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
                        .map_err(|_| qihe_cancelled())?,
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

        let message = diagnostic_message(message, &element, location_unknown, extra_support_lines);
        let line_info = self.snapshot.line_info(file_id).map_err(|_| qihe_cancelled())?;
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
            resolve_file_name(self.default_path, name)
                .and_then(|path| self.snapshot.file_id_for_path(path.as_ref()))
        });
        let Some(file_id) = file_id else { return Ok(None) };

        let line_index =
            self.snapshot.analysis.line_index(file_id).map_err(|_| qihe_cancelled())?;
        let line = location.line.saturating_sub(1);
        let col = location.column.saturating_sub(1);
        let Some(offset) = line_index.offset(LineCol { line, col }) else {
            return Ok(None);
        };

        Ok(Some((file_id, TextRange::empty(offset))))
    }
}

fn diagnostic_message(
    message: String,
    primary_element: &str,
    location_unknown: bool,
    mut extra_support_lines: Vec<String>,
) -> String {
    if location_unknown && !primary_element.is_empty() {
        extra_support_lines.push(format!("Location: {primary_element}"));
    }
    extra_support_lines.insert(0, message);
    extra_support_lines.join("\n")
}

fn analysis_code(analysis_class: &str) -> String {
    analysis_class.rsplit('.').next().filter(|code| !code.is_empty()).unwrap_or("Qihe").to_owned()
}

fn qihe_cancelled() -> anyhow::Error {
    anyhow!("qihe analysis cancelled")
}

fn resolve_file_name(default_path: &AbsPath, file_name: &str) -> Option<PathBuf> {
    let candidate = Path::new(file_name);
    Some(if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        default_path.parent()?.join(file_name).into()
    })
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
    use super::{parse_source_loc, strip_ansi};

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
}
