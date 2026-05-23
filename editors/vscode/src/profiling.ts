import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import * as vscode from 'vscode';

import { stripProfileArgs } from './profilingArgs';
import {
  type DiagnosticProfileRequest,
  diagnosticsFromProfileResponse,
  summarizeDiagnostics,
} from './profilingDiagnostics';
import { LspProfileSession } from './profilingSession';
import { summarizeTraceFile } from './profilingTrace';
import type {
  ProfileArtifacts,
  ProfileRunSummary,
  ProfileTarget,
  ProfilingDependencies,
  ServerLaunch,
} from './profilingTypes';

export const profileDiagnosticsCommand = 'vizsla.profileDiagnostics';

const profileOutputChannelName = 'Vizsla Profiling';
const profileTimeoutMs = 120_000;
const shutdownTimeoutMs = 15_000;
const defaultLogFilter = 'vizsla=info,base_db=info,ide=info,project_model=info,hir=info';
const defaultTraceFilter = [
  'vizsla=trace',
  'base_db=trace',
  'hir=trace',
  'ide=trace',
  'project_model=trace',
  'slang=trace',
  'utils=trace',
  'vfs=trace',
  'vfs_notify=trace',
].join(',');

export function registerProfilingCommand(
  context: vscode.ExtensionContext,
  deps: ProfilingDependencies,
): vscode.Disposable {
  const channel = vscode.window.createOutputChannel(profileOutputChannelName);
  context.subscriptions.push(channel);

  return vscode.commands.registerCommand(profileDiagnosticsCommand, async () => {
    await runProfileDiagnostics(context, deps, channel);
  });
}

async function runProfileDiagnostics(
  context: vscode.ExtensionContext,
  deps: ProfilingDependencies,
  channel: vscode.OutputChannel,
): Promise<void> {
  const target = await selectProfileTarget();
  if (!target) {
    return;
  }

  const artifacts = profileArtifacts(context, target);
  channel.clear();
  channel.show(true);
  channel.appendLine(`Profiling ${profileTargetLabel(target)}`);
  channel.appendLine(`Artifacts: ${artifacts.dir}`);

  let session: LspProfileSession | undefined;
  try {
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: vscode.l10n.t('Running Vizsla diagnostics profile'),
        cancellable: true,
      },
      async (_progress, token) => {
        token.onCancellationRequested(() => session?.dispose());
        await runProfileSession(
          target,
          artifacts,
          deps.resolveLaunch(),
          deps,
          channel,
          (activeSession) => {
            session = activeSession;
          },
        );
      },
    );

    await showProfileCompleteMessage(artifacts);
  } catch (error) {
    const message = vscode.l10n.t(
      'Failed to profile diagnostics: {0}',
      (error as Error).message,
    );
    channel.appendLine(`[ERROR] ${message}`);
    vscode.window.showErrorMessage(message);
  } finally {
    session?.dispose();
  }
}

type ProfileTargetPick = vscode.QuickPickItem & { target: ProfileTarget };

async function selectProfileTarget(): Promise<ProfileTarget | undefined> {
  const options = profileTargetOptions();
  if (options.length === 0) {
    vscode.window.showWarningMessage(
      vscode.l10n.t('Open a workspace or a Verilog/SystemVerilog file first.'),
    );
    return undefined;
  }

  if (options.length === 1) {
    return options[0].target;
  }

  const picked = await vscode.window.showQuickPick(options, {
    placeHolder: vscode.l10n.t('Select diagnostics profile target'),
  });
  return picked?.target;
}

function profileTargetOptions(): ProfileTargetPick[] {
  const options: ProfileTargetPick[] = [];
  for (const workspaceFolder of vscode.workspace.workspaceFolders ?? []) {
    if (workspaceFolder.uri.scheme !== 'file') {
      continue;
    }
    options.push({
      label: vscode.l10n.t('Workspace Diagnostics'),
      description: workspaceFolder.name,
      detail: vscode.l10n.t('Runs workspace/diagnostic for {0}', workspaceFolder.uri.fsPath),
      target: {
        scope: 'workspace',
        workspaceRoot: workspaceFolder.uri.fsPath,
        workspaceName: workspaceFolder.name || path.basename(workspaceFolder.uri.fsPath),
      },
    });
  }

  const documentTarget = currentDocumentProfileTarget();
  if (documentTarget?.scope === 'document') {
    options.push({
      label: vscode.l10n.t('Current File Diagnostics'),
      description: path.basename(documentTarget.document.uri.fsPath),
      detail: vscode.l10n.t(
        'Runs textDocument/diagnostic for {0}',
        documentTarget.document.uri.fsPath,
      ),
      target: documentTarget,
    });
  }

  return options;
}

function currentDocumentProfileTarget(): ProfileTarget | undefined {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    return undefined;
  }

  const { document } = editor;
  if (!['verilog', 'systemverilog'].includes(document.languageId)) {
    return undefined;
  }

  if (document.uri.scheme !== 'file') {
    return undefined;
  }

  const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
  const workspaceRoot = workspaceFolder?.uri.fsPath ?? path.dirname(document.uri.fsPath);
  const workspaceName = (workspaceFolder?.name ?? path.basename(workspaceRoot)) || 'workspace';
  return { scope: 'document', document, workspaceRoot, workspaceName };
}

function profileArtifacts(context: vscode.ExtensionContext, target: ProfileTarget): ProfileArtifacts {
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const stem = profileArtifactStem(target).replace(/[^\w.-]+/g, '_');
  const dir = path.join(context.globalStorageUri.fsPath, 'profiles', `${timestamp}-${stem}`);
  return {
    dir,
    trace: path.join(dir, 'trace.json'),
    summary: path.join(dir, 'summary.json'),
    folded: path.join(dir, 'trace.folded'),
    html: path.join(dir, 'flamegraph.html'),
    svg: path.join(dir, 'flamegraph.svg'),
    log: path.join(dir, 'server.log'),
  };
}

function profileArtifactStem(target: ProfileTarget): string {
  if (target.scope === 'document') {
    return path.basename(target.document.uri.fsPath, path.extname(target.document.uri.fsPath));
  }
  return `workspace-${target.workspaceName || path.basename(target.workspaceRoot)}`;
}

function profileTargetLabel(target: ProfileTarget): string {
  if (target.scope === 'document') {
    return `${target.document.uri.fsPath} (${diagnosticRequestForTarget(target).method})`;
  }
  return `${target.workspaceRoot} (${diagnosticRequestForTarget(target).method})`;
}

function diagnosticRequestForTarget(target: ProfileTarget): {
  method: DiagnosticProfileRequest;
  params: unknown;
} {
  if (target.scope === 'workspace') {
    return {
      method: 'workspace/diagnostic',
      params: { previousResultIds: [] },
    };
  }

  return {
    method: 'textDocument/diagnostic',
    params: { textDocument: { uri: target.document.uri.toString() } },
  };
}

async function runProfileSession(
  target: ProfileTarget,
  artifacts: ProfileArtifacts,
  launch: ServerLaunch,
  deps: ProfilingDependencies,
  channel: vscode.OutputChannel,
  setActiveSession: (session: LspProfileSession) => void,
): Promise<void> {
  await fs.promises.mkdir(artifacts.dir, { recursive: true });
  const started = Date.now();
  const session = createProfileSession(launch, artifacts, deps, channel);
  setActiveSession(session);

  try {
    await session.initialize(target, profileTimeoutMs);
    const diagnosticRequest = diagnosticRequestForTarget(target);
    const requestStarted = Date.now();
    const response = await session.request(
      diagnosticRequest.method,
      diagnosticRequest.params,
      profileTimeoutMs,
    );
    const requestElapsedMs = Date.now() - requestStarted;
    const diagnostics = diagnosticsFromProfileResponse(response, diagnosticRequest.method);

    await stopProfileSession(session, channel);

    const summary: ProfileRunSummary = {
      scope: target.scope,
      request: diagnosticRequest.method,
      ...(target.scope === 'document' ? { file: target.document.uri.fsPath } : {}),
      workspace: target.workspaceRoot,
      elapsed_ms: Date.now() - started,
      diagnostic_request_ms: requestElapsedMs,
      diagnostics: summarizeDiagnostics(diagnostics),
      artifacts: {
        trace: artifacts.trace,
        folded: artifacts.folded,
        flamegraph_html: artifacts.html,
        flamegraph: artifacts.svg,
        server_log: artifacts.log,
      },
      trace_summary: await summarizeTraceFile(
        artifacts.trace,
        artifacts.folded,
        artifacts.svg,
        artifacts.html,
      ),
    };
    await writeJsonFile(artifacts.summary, summary);
    channel.appendLine(`Request: ${diagnosticRequest.method}`);
    channel.appendLine(`Diagnostic request: ${requestElapsedMs} ms`);
    channel.appendLine(`Diagnostics: ${diagnostics.length}`);
    channel.appendLine(`Summary: ${artifacts.summary}`);
    channel.appendLine(`Interactive flamegraph: ${artifacts.html}`);
    channel.appendLine(`Static flamegraph: ${artifacts.svg}`);
  } finally {
    session.dispose();
  }
}

function createProfileSession(
  launch: ServerLaunch,
  artifacts: ProfileArtifacts,
  deps: ProfilingDependencies,
  channel: vscode.OutputChannel,
): LspProfileSession {
  const serverArgs = [
    ...stripProfileArgs([...launch.args, ...launch.additionalArgs]),
    '--log',
    defaultLogFilter,
    '--log_file',
    artifacts.log,
    '--profile_trace',
    artifacts.trace,
  ];
  channel.appendLine(`Server: ${launch.command}`);
  channel.appendLine(`Working directory: ${launch.cwd}`);

  const child = spawn(launch.command, serverArgs, {
    cwd: launch.cwd,
    env: {
      ...deps.createEnv(),
      VIZSLA_PROFILE_TRACE_FILTER: defaultTraceFilter,
    },
    stdio: 'pipe',
  });

  return new LspProfileSession(child, channel, readVizslaInitializationOptions);
}

async function stopProfileSession(
  session: LspProfileSession,
  channel: vscode.OutputChannel,
): Promise<void> {
  await session.request('shutdown', null, shutdownTimeoutMs).catch((error: unknown) => {
    channel.appendLine(`[WARN] Shutdown request failed: ${(error as Error).message}`);
  });
  session.notify('exit', {});
  await session.waitForExit(shutdownTimeoutMs);
}

async function showProfileCompleteMessage(artifacts: ProfileArtifacts): Promise<void> {
  const openFlamegraph = vscode.l10n.t('Open Flamegraph');
  const openSummary = vscode.l10n.t('Open Summary');
  const showInFolder = vscode.l10n.t('Show in Folder');
  const selection = await vscode.window.showInformationMessage(
    vscode.l10n.t('Vizsla diagnostics profile complete.'),
    openFlamegraph,
    openSummary,
    showInFolder,
  );
  if (selection === openFlamegraph) {
    await vscode.env.openExternal(vscode.Uri.file(artifacts.html));
  } else if (selection === openSummary) {
    await vscode.window.showTextDocument(vscode.Uri.file(artifacts.summary));
  } else if (selection === showInFolder) {
    await vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(artifacts.summary));
  }
}

function readVizslaInitializationOptions(): Record<string, unknown> {
  const config = vscode.workspace.getConfiguration('vizsla');
  return {
    files_excludeDirs: config.get('files.excludeDirs') ?? [],
    files_watcher: config.get('files.watcher') ?? 'client',
    workspace_auto_reload: config.get('workspace.auto.reload') ?? true,
    scope_visibility: config.get('scope.visibility') ?? 'private',
    formatter_provider: config.get('formatter.provider') ?? 'verible',
    formatter_path: config.get('formatter.path') ?? null,
    formatter_args: config.get('formatter.args') ?? ['--failsafe_success=false'],
    formatting_on_enter: config.get('formatting.on.enter') ?? true,
    formatting_in_comments: config.get('formatting.in.comments') ?? true,
    formatting_indent_width: config.get('formatting.indent.width') ?? 4,
    inlayHints_port_connection_enable: config.get('inlayHints.port.connection.enable') ?? true,
    inlayHints_parameter_assignment_enable:
      config.get('inlayHints.parameter.assignment.enable') ?? true,
    inlayHints_end_structure_enable: config.get('inlayHints.end.structure.enable') ?? true,
    lens_instantiations_enable: config.get('lens.instantiations.enable') ?? true,
    semantic_tokens_port_clk_rst_enable:
      config.get('semantic.tokens.port.clk.rst.enable') ?? true,
    semantic_tokens_port_input_output_enable:
      config.get('semantic.tokens.port.input.output.enable') ?? true,
    diagnostics: {
      enable: config.get('diagnostics.enable') ?? true,
      update: config.get('diagnostics.update') ?? 'onSave',
      parse: { enable: config.get('diagnostics.parse.enable') ?? true },
      semantic: { enable: config.get('diagnostics.semantic.enable') ?? true },
      slang: {
        warnings: config.get('diagnostics.slang.warnings') ?? [],
        rules: config.get('diagnostics.slang.rules') ?? [],
      },
    },
    signature_help_params_only: config.get('signature.help.params.only') ?? false,
    qihe_command: config.get('qihe.command') ?? 'qihe',
    qihe_compileArgs: config.get('qihe.compileArgs') ?? [],
    qihe_runArgs: config.get('qihe.runArgs') ?? ['-g', 'std'],
  };
}

async function writeJsonFile(filePath: string, value: unknown): Promise<void> {
  await fs.promises.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}
