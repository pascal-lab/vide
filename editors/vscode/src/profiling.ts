import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import * as vscode from 'vscode';

import { stripProfileArgs } from './profilingArgs';
import { diagnosticsProfilingInitializationOptions } from './profilingConfig';
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
import { SpeedscopeProfileViewer } from './profilingViewer';

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
  const viewer = new SpeedscopeProfileViewer(context);
  context.subscriptions.push(channel);
  context.subscriptions.push(viewer);

  return vscode.commands.registerCommand(profileDiagnosticsCommand, async () => {
    await runProfileDiagnostics(context, deps, channel, viewer);
  });
}

async function runProfileDiagnostics(
  context: vscode.ExtensionContext,
  deps: ProfilingDependencies,
  channel: vscode.OutputChannel,
  viewer: SpeedscopeProfileViewer,
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

    await showProfileCompleteMessage(artifacts, viewer);
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
        flamegraph_svg: artifacts.svg,
        server_log: artifacts.log,
      },
      trace_summary: await summarizeTraceFile(artifacts.trace, artifacts.folded, artifacts.svg),
    };
    await writeJsonFile(artifacts.summary, summary);
    channel.appendLine(`Request: ${diagnosticRequest.method}`);
    channel.appendLine(`Diagnostic request: ${requestElapsedMs} ms`);
    channel.appendLine(`Diagnostics: ${diagnostics.length}`);
    channel.appendLine(`Summary: ${artifacts.summary}`);
    channel.appendLine(`Speedscope input trace: ${artifacts.trace}`);
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

  return new LspProfileSession(child, channel, readDiagnosticsProfilingInitializationOptions);
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

async function showProfileCompleteMessage(
  artifacts: ProfileArtifacts,
  viewer: SpeedscopeProfileViewer,
): Promise<void> {
  const openSpeedscope = vscode.l10n.t('Open in Speedscope');
  const openSummary = vscode.l10n.t('Open Summary');
  const showInFolder = vscode.l10n.t('Show in Folder');
  const selection = await vscode.window.showInformationMessage(
    vscode.l10n.t('Vizsla diagnostics profile complete.'),
    openSpeedscope,
    openSummary,
    showInFolder,
  );
  if (selection === openSpeedscope) {
    await viewer.open(artifacts).catch((error: unknown) => {
      vscode.window.showErrorMessage(
        vscode.l10n.t('Failed to open Speedscope: {0}', (error as Error).message),
      );
    });
  } else if (selection === openSummary) {
    await vscode.window.showTextDocument(vscode.Uri.file(artifacts.summary));
  } else if (selection === showInFolder) {
    await vscode.commands.executeCommand('revealFileInOS', vscode.Uri.file(artifacts.summary));
  }
}

function readDiagnosticsProfilingInitializationOptions(): Record<string, unknown> {
  return diagnosticsProfilingInitializationOptions(vscode.workspace.getConfiguration('vizsla'));
}

async function writeJsonFile(filePath: string, value: unknown): Promise<void> {
  await fs.promises.writeFile(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf8');
}
