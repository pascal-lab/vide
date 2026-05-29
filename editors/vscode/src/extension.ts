import { execFile } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { promisify } from 'node:util';

import * as vscode from 'vscode';
import {
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { getBundledServerPath, getPlatformFolder } from './platform';
import { registerDiagnosticActions } from './diagnosticActions';
import { profileDiagnosticsCommand, registerProfilingCommand } from './profiling';
import { serverInitializationOptions } from './initializationOptions';
import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_FILE_NAMES,
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_SOURCE_FILE_GLOB,
  getProjectConfigPath,
} from './projectConfig';
import {
  projectStatusNotification,
  reloadWorkspaceCommand,
  reloadWorkspaceRequest,
  showOutputCommand,
  showStatusCommand,
  VideStatusController,
} from './videStatus';
import type { ServerStatus } from './status';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let qiheOutputChannel: vscode.OutputChannel | undefined;
let videStatusController: VideStatusController | undefined;
let qiheStatusBarItem: vscode.StatusBarItem | undefined;

const execFileAsync = promisify(execFile);
const restartServerCommand = 'vide.restartServer';
const showServerVersionCommand = 'vide.showServerVersion';
const showQiheOutputCommand = 'vide.showQiheOutput';
const runQiheAnalysisCommand = 'vide.runQiheAnalysis';
const runQiheAnalysisRequest = 'vide.server.runQiheAnalysis';
const qiheStatusNotification = 'vide/qiheStatus';
const qiheLogNotification = 'vide/qiheLog';
const qiheAnalysisIcon = '$(beaker)';
// Output channel names are stable identifiers in the Output view.
const languageServerOutputChannelName = 'Vide Language Server';
const qiheOutputChannelName = 'Vide Qihe';
const versionTimeoutMs = 5000;

interface ExtensionBuildInfo {
  kind?: string;
  commitHash?: string;
  buildDate?: string;
}

const activeQiheTokens = new Set<string>();
const qiheProgressNotifications = new Map<string, { resolve: () => void }>();
let qiheStatusHideTimer: NodeJS.Timeout | undefined;

function log(message: string): void {
  outputChannel?.appendLine(message);
}

function logQihe(message: string): void {
  qiheOutputChannel?.appendLine(message);
}

function requireOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    throw new Error(vscode.l10n.t('Vide output channel has not been initialized.'));
  }

  return outputChannel;
}

function requireQiheOutputChannel(): vscode.OutputChannel {
  if (!qiheOutputChannel) {
    throw new Error(vscode.l10n.t('Vide Qihe output channel has not been initialized.'));
  }

  return qiheOutputChannel;
}

function showOutput(): void {
  requireOutputChannel().show(true);
}

function showQiheOutput(): void {
  requireQiheOutputChannel().show(true);
}

function extensionVersion(context: vscode.ExtensionContext): string {
  const packageJson = context.extension.packageJSON as { version?: unknown };
  return typeof packageJson.version === 'string' && packageJson.version.length > 0
    ? packageJson.version
    : 'unknown';
}

function extensionBuildInfo(context: vscode.ExtensionContext): ExtensionBuildInfo | undefined {
  const buildInfoPath = path.join(context.extensionPath, 'build-info.json');
  if (!fs.existsSync(buildInfoPath)) {
    return undefined;
  }
  return JSON.parse(fs.readFileSync(buildInfoPath, 'utf8')) as ExtensionBuildInfo;
}

function extensionBuildLabel(context: vscode.ExtensionContext): string {
  const version = extensionVersion(context);
  const buildInfo = extensionBuildInfo(context);
  const details = [buildInfo?.kind, buildInfo?.commitHash, buildInfo?.buildDate].filter(
    (part): part is string => typeof part === 'string' && part.length > 0,
  );
  return details.length > 0 ? `${version} (${details.join(', ')})` : version;
}

async function showLanguageServerErrorMessage(message: string): Promise<void> {
  const showOutputAction = vscode.l10n.t('Show Output');
  const selection = await vscode.window.showErrorMessage(message, showOutputAction);
  if (selection === showOutputAction) {
    showOutput();
  }
}

async function showQiheErrorMessage(message: string): Promise<void> {
  const showOutputAction = vscode.l10n.t('Show Qihe Output');
  const selection = await vscode.window.showErrorMessage(message, showOutputAction);
  if (selection === showOutputAction) {
    showQiheOutput();
  }
}

function updateServerStatus(status: ServerStatus, detail?: string): void {
  videStatusController?.updateServerStatus(status, detail);
}

function clearQiheStatusHideTimer(): void {
  if (!qiheStatusHideTimer) {
    return;
  }

  clearTimeout(qiheStatusHideTimer);
  qiheStatusHideTimer = undefined;
}

function updateQiheStatus(
  tooltip: string,
  hideAfterMs?: number,
  command: string | vscode.Command = runQiheAnalysisCommand,
): void {
  if (!qiheStatusBarItem) {
    return;
  }

  clearQiheStatusHideTimer();
  qiheStatusBarItem.text = `${qiheAnalysisIcon} Qihe`;
  qiheStatusBarItem.tooltip = tooltip;
  qiheStatusBarItem.command = command;
  qiheStatusBarItem.show();

  if (!hideAfterMs) {
    return;
  }

  qiheStatusHideTimer = setTimeout(() => {
    qiheStatusBarItem?.hide();
    qiheStatusHideTimer = undefined;
  }, hideAfterMs);
}

function createQiheStatusBarItem(): vscode.StatusBarItem {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.name = vscode.l10n.t('Qihe Analysis Status');
  item.command = runQiheAnalysisCommand;
  item.hide();
  return item;
}

function startQiheNotification(token: string, message?: string): void {
  if (qiheProgressNotifications.has(token)) {
    return;
  }

  let resolveProgress = () => {};
  const progressPromise = new Promise<void>((resolve) => {
    resolveProgress = resolve;
  });

  qiheProgressNotifications.set(token, { resolve: resolveProgress });

  void vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: vscode.l10n.t('Running Qihe analysis'),
    },
    async (progress) => {
      if (message) {
        progress.report({ message });
      }
      await progressPromise;
    },
  );
}

function finishQiheNotification(token: string): void {
  const entry = qiheProgressNotifications.get(token);
  if (!entry) {
    return;
  }

  qiheProgressNotifications.delete(token);
  entry.resolve();
}

function registerQiheNotifications(languageClient: LanguageClient): void {
  languageClient.onNotification(
    qiheLogNotification,
    (params: { token?: unknown; message?: unknown }) => {
      const message =
        typeof params.message === 'string' ? params.message : undefined;

      if (!message) {
        return;
      }

      logQihe(message);
    },
  );

  languageClient.onNotification(
    qiheStatusNotification,
    (params: { token?: unknown; state?: unknown; message?: unknown }) => {
      const token =
        typeof params.token === 'string' ? params.token : undefined;
      const state =
        typeof params.state === 'string' ? params.state : undefined;
      const message =
        typeof params.message === 'string' ? params.message : undefined;

      if (!token || !state) {
        return;
      }

      switch (state) {
        case 'begin':
          activeQiheTokens.add(token);
          updateQiheStatus(message ?? vscode.l10n.t('Qihe analysis is running'));
          startQiheNotification(token, message);
          break;
        case 'end':
          activeQiheTokens.delete(token);
          finishQiheNotification(token);
          if (activeQiheTokens.size === 0) {
            updateQiheStatus(message ?? vscode.l10n.t('Qihe analysis finished'), 4000);
          }
          break;
        case 'failed':
          activeQiheTokens.delete(token);
          finishQiheNotification(token);
          if (activeQiheTokens.size === 0) {
            const failureMessage = message ?? vscode.l10n.t('Qihe analysis failed');
            updateQiheStatus(failureMessage, 6000, showQiheOutputCommand);
            void showQiheErrorMessage(failureMessage);
          }
          break;
        default:
          break;
      }
    },
  );
}

function registerProjectStatusNotifications(languageClient: LanguageClient): void {
  languageClient.onNotification(projectStatusNotification, (params: unknown) => {
    videStatusController?.handleProjectNotification(params);
  });
}

interface ServerConfiguration {
  command: string | undefined;
  args: string[];
  additionalArgs: string[];
  cwd: string | undefined;
  trace: 'off' | 'messages' | 'verbose';
}

interface ServerLaunch {
  command: string;
  args: string[];
  additionalArgs: string[];
  cwd: string;
}

function asStringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;
}

function getServerPath(context: vscode.ExtensionContext): string | undefined {
  const platform = process.platform;
  const arch = process.arch;
  const platformFolder = getPlatformFolder(platform, arch);
  if (!platformFolder) {
    log(
      `[ERROR] Unsupported platform-architecture combination: ${platform}-${arch}`,
    );
    return undefined;
  }

  const bundledPath = getBundledServerPath(context.extensionPath, platform, arch);
  if (!bundledPath) {
    log(`[ERROR] Unsupported platform-architecture combination: ${platformFolder}`);
    return undefined;
  }

  log(`[INFO] Looking for bundled server at: ${bundledPath}`);

  if (fs.existsSync(bundledPath)) {
    if (platform !== 'win32') {
      try {
        fs.accessSync(bundledPath, fs.constants.X_OK);
        log('[INFO] Bundled server binary is executable');
        return bundledPath;
      } catch {
        log(
          '[WARN] Bundled server binary exists but is not executable, attempting to fix...',
        );
        try {
          fs.chmodSync(bundledPath, 0o755);
          log('[INFO] Made bundled server binary executable');
          return bundledPath;
        } catch (error) {
          log(
            `[ERROR] Failed to make bundled binary executable: ${(error as Error).message}`,
          );
        }
      }
    } else {
      log('[INFO] Found bundled server binary');
      return bundledPath;
    }
  } else {
    log(`[INFO] Bundled server binary not found at: ${bundledPath}`);
  }

  return undefined;
}

function readConfiguration(): ServerConfiguration {
  const config = vscode.workspace.getConfiguration('vide');
  const command = config.get<string | null>('server.command');
  const args = asStringArray(config.get<unknown>('server.args'));
  const additionalArgs = asStringArray(config.get<unknown>('server.additionalArgs'));
  const cwd = config.get<string | null>('server.cwd');
  const trace = config.get<'off' | 'messages' | 'verbose'>('trace.server') ?? 'off';

  if (!args || !additionalArgs) {
    vscode.window.showErrorMessage(
      vscode.l10n.t('vide server arguments settings must be arrays of strings.'),
    );
    return {
      command: undefined,
      args: [],
      additionalArgs: [],
      cwd: undefined,
      trace,
    };
  }

  return {
    command: typeof command === 'string' && command.length > 0 ? command : undefined,
    args,
    additionalArgs,
    cwd: typeof cwd === 'string' && cwd.length > 0 ? cwd : undefined,
    trace,
  };
}

function includeDeclarationInReferences(document: vscode.TextDocument): boolean {
  return (
    vscode.workspace
      .getConfiguration('vide', document)
      .get<boolean>('references.includeDeclaration') ?? true
  );
}

function resolveWorkingDirectory(
  context: vscode.ExtensionContext,
  configuredCwd: string | undefined,
): string {
  if (configuredCwd) {
    log(`[INFO] Using configured working directory: ${configuredCwd}`);
    return configuredCwd;
  }

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (workspaceFolder) {
    const workspacePath = workspaceFolder.uri.fsPath;
    log(`[INFO] Using workspace folder as working directory: ${workspacePath}`);
    return workspacePath;
  }

  log(`[INFO] Using extension path as working directory: ${context.extensionPath}`);
  return context.extensionPath;
}

function resolveServerLaunch(
  context: vscode.ExtensionContext,
  config: ServerConfiguration,
): ServerLaunch {
  const cwd = resolveWorkingDirectory(context, config.cwd);

  let serverCommand = config.command;
  if (!serverCommand) {
    serverCommand = getServerPath(context);
    if (!serverCommand) {
      const message = vscode.l10n.t(
        'Bundled Vide Language Server binary not found. Install the VSIX that matches your platform or configure "vide.server.command".',
      );
      log(`[ERROR] ${message}`);
      throw new Error(message);
    }
  } else {
    log(`[INFO] Using custom server command: ${serverCommand}`);
  }

  log(`[INFO] Server command: ${serverCommand}`);
  log(`[INFO] Server args: ${JSON.stringify([...config.args, ...config.additionalArgs])}`);
  log(`[INFO] Working directory: ${cwd}`);

  return {
    command: serverCommand,
    args: config.args,
    additionalArgs: config.additionalArgs,
    cwd,
  };
}

function createServerEnv(
  logLevel: 'info' | 'debug' = 'info',
  backtrace: '1' | 'full' = '1',
): NodeJS.ProcessEnv {
  return {
    ...process.env,
    RUST_BACKTRACE: backtrace,
    RUST_LOG: logLevel,
  };
}

type ProjectConfigTarget = {
  folderName: string;
  configPath: string;
};

async function findMissingProjectConfigTargets(): Promise<ProjectConfigTarget[]> {
  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  const targets: ProjectConfigTarget[] = [];

  for (const folder of workspaceFolders) {
    if (folder.uri.scheme !== 'file') {
      log(
        `[WARN] Skipping project config creation for non-file workspace: ${folder.uri.toString()}`,
      );
      continue;
    }

    const existingConfigPath = PROJECT_CONFIG_FILE_NAMES
      .map((fileName) => getProjectConfigPath(folder.uri.fsPath, fileName))
      .find((configPath) => fs.existsSync(configPath));
    if (existingConfigPath) {
      log(`[INFO] Found project config: ${existingConfigPath}`);
      continue;
    }

    const sourceFiles = await vscode.workspace.findFiles(
      new vscode.RelativePattern(folder, PROJECT_SOURCE_FILE_GLOB),
      undefined,
      1,
    );
    if (sourceFiles.length === 0) {
      log(
        `[INFO] Skipping project config prompt for workspace without Verilog/SystemVerilog files: ${folder.name}`,
      );
      continue;
    }

    const configPath = getProjectConfigPath(folder.uri.fsPath);
    targets.push({ folderName: folder.name, configPath });
  }

  return targets;
}

function projectConfigTargetsFromRootUris(rootUris: readonly string[]): ProjectConfigTarget[] {
  return rootUris.map((rootUri) => {
    const uri = vscode.Uri.parse(rootUri);
    return {
      folderName: path.basename(uri.fsPath),
      configPath: getProjectConfigPath(uri.fsPath),
    };
  });
}

async function promptForMissingProjectConfigs(context: vscode.ExtensionContext): Promise<void> {
  const targets = await findMissingProjectConfigTargets();

  if (targets.length === 0) {
    return;
  }

  const createConfigAction =
    targets.length === 1
      ? vscode.l10n.t('Create Manifest')
      : vscode.l10n.t('Create Manifests');
  const restartNotice = vscode.l10n.t(
    'Creating a manifest will restart the Vide language server so the workspace can reload it.',
  );
  const promptMessage =
    targets.length === 1
      ? vscode.l10n.t(
          'No Vide project manifest was detected in {0}. Project-aware features like semantic diagnostics, navigation, and references may be severely limited. {1}',
          targets[0].folderName,
          restartNotice,
        )
      : vscode.l10n.t(
          'No Vide project manifest was detected in {0} workspace folders. Project-aware features like semantic diagnostics, navigation, and references may be severely limited. {1}',
          targets.length,
          restartNotice,
        );

  const selection = await vscode.window.showWarningMessage(promptMessage, createConfigAction);
  if (selection !== createConfigAction) {
    return;
  }

  await createProjectConfigs(context, targets);
}

async function createProjectConfigsFromRootUris(
  context: vscode.ExtensionContext,
  rootUris: readonly string[],
): Promise<void> {
  await createProjectConfigs(context, projectConfigTargetsFromRootUris(rootUris));
}

async function createProjectConfigs(
  context: vscode.ExtensionContext,
  targets: readonly ProjectConfigTarget[],
): Promise<void> {
  if (targets.length === 0) {
    return;
  }

  const createdConfigs: vscode.Uri[] = [];

  for (const { folderName, configPath } of targets) {
    try {
      await fs.promises.writeFile(configPath, DEFAULT_PROJECT_CONFIG_TEXT, {
        encoding: 'utf8',
        flag: 'wx',
      });
      createdConfigs.push(vscode.Uri.file(configPath));
      log(`[INFO] Created default project config: ${configPath}`);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'EEXIST') {
        log(`[INFO] Project config already exists: ${configPath}`);
        continue;
      }

      const errorMessage = vscode.l10n.t(
        'Failed to create {0} in {1}: {2}',
        PROJECT_CONFIG_FILE_NAME,
        folderName,
        (error as Error).message,
      );
      log(`[WARN] ${errorMessage}`);
      void vscode.window.showWarningMessage(errorMessage);
    }
  }

  if (createdConfigs.length === 0) {
    return;
  }

  if (client) {
    await restartClient(context);
  }

  const createdMessage =
    createdConfigs.length === 1
      ? vscode.l10n.t('Created {0}.', PROJECT_CONFIG_FILE_NAME)
      : vscode.l10n.t(
          'Created {0} in {1} workspace folders.',
          PROJECT_CONFIG_FILE_NAME,
          createdConfigs.length,
        );
  const openConfigAction =
    createdConfigs.length === 1
      ? vscode.l10n.t('Open Manifest')
      : vscode.l10n.t('Open First Manifest');

  void vscode.window.showInformationMessage(createdMessage, openConfigAction).then(async (selection) => {
    if (selection !== openConfigAction) {
      return;
    }

    try {
      await vscode.window.showTextDocument(createdConfigs[0]);
    } catch (error) {
      log(`[WARN] Failed to open ${PROJECT_CONFIG_FILE_NAME}: ${(error as Error).message}`);
    }
  });
}

async function createClient(context: vscode.ExtensionContext): Promise<LanguageClient> {
  const channel = requireOutputChannel();
  log('[INFO] Creating language client...');

  const config = readConfiguration();
  const launch = resolveServerLaunch(context, config);
  const serverArgs = [...launch.args, ...launch.additionalArgs];

  const commonEnv = {
    ...createServerEnv(),
  };

  const serverOptions: ServerOptions = {
    run: {
      command: launch.command,
      args: serverArgs,
      options: { cwd: launch.cwd, env: commonEnv },
    },
    debug: {
      command: launch.command,
      args: serverArgs,
      options: {
        cwd: launch.cwd,
        env: createServerEnv('debug', 'full'),
      },
    },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'verilog' },
      { scheme: 'file', language: 'systemverilog' },
    ],
    synchronize: {
      configurationSection: ['vide'],
    },
    outputChannel: channel,
    traceOutputChannel: channel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationOptions: serverInitializationOptions(vscode.workspace.getConfiguration('vide')),
    middleware: {
      provideReferences: async (document, position, options, token, next) => {
        options.includeDeclaration = includeDeclarationInReferences(document);
        return await next(document, position, options, token);
      },
    },
    ...(config.trace !== 'off' && { trace: config.trace }),
  };

  log('[INFO] Creating LanguageClient instance...');
  return new LanguageClient(
    'vide',
    vscode.l10n.t('Vide Language Server'),
    serverOptions,
    clientOptions,
  );
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  try {
    updateServerStatus('starting');
    log('[INFO] Starting language server...');
    client = await createClient(context);
    registerProjectStatusNotifications(client);
    registerQiheNotifications(client);
    await client.start();
    log('[INFO] Language server started successfully');
    updateServerStatus('ready');
  } catch (error) {
    const message = (error as Error).message;
    client = undefined;
    log(`[ERROR] Failed to start language server: ${message}`);
    log(`[ERROR] ${(error as Error).stack}`);
    updateServerStatus('error', message);
    await showLanguageServerErrorMessage(
      vscode.l10n.t('Failed to start Vide Language Server: {0}', message),
    );
  }
}

async function stopClient(): Promise<void> {
  if (!client) {
    updateServerStatus('stopped');
    return;
  }

  updateServerStatus('stopping');
  log('[INFO] Stopping language server...');
  try {
    await client.stop();
    log('[INFO] Language server stopped');
  } catch (error) {
    log(`[ERROR] Error stopping language server: ${(error as Error).message}`);
  } finally {
    client = undefined;
    updateServerStatus('stopped');
  }
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  log('[INFO] Restarting language server...');
  await stopClient();
  await startClient(context);
}

async function showServerVersion(context: vscode.ExtensionContext): Promise<void> {
  try {
    const config = readConfiguration();
    const launch = resolveServerLaunch(context, config);
    const versionArgs = [...launch.args, '--version'];
    log(`[INFO] Checking server version: ${launch.command} ${versionArgs.join(' ')}`);
    const { stdout, stderr } = await execFileAsync(launch.command, versionArgs, {
      cwd: launch.cwd,
      env: createServerEnv(),
      timeout: versionTimeoutMs,
    });
    const output = `${stdout}${stderr}`.trim() || vscode.l10n.t('No version output');
    const firstLine = output.split(/\r?\n/, 1)[0] ?? output;
    log(`[INFO] Server version output:\n${output}`);
    vscode.window.showInformationMessage(
      vscode.l10n.t(
        'Vide extension: {0}; server: {1}',
        extensionBuildLabel(context),
        firstLine,
      ),
    );
  } catch (error) {
    const message = vscode.l10n.t(
      'Failed to query Vide server version: {0}',
      (error as Error).message,
    );
    log(`[ERROR] ${message}`);
    await showLanguageServerErrorMessage(message);
  }
}

async function reloadWorkspace(): Promise<void> {
  if (!client) {
    await showLanguageServerErrorMessage(vscode.l10n.t('Vide language server is not running.'));
    return;
  }

  try {
    await client.sendRequest('workspace/executeCommand', {
      command: reloadWorkspaceRequest,
      arguments: [],
    });
  } catch (error) {
    const message = vscode.l10n.t(
      'Failed to reload Vide project configuration: {0}',
      (error as Error).message,
    );
    log(`[ERROR] ${message}`);
    await showLanguageServerErrorMessage(message);
  }
}

async function runQiheAnalysis(resource: unknown): Promise<void> {
  const targetUri = qiheAnalysisTargetUri(resource);
  if (!targetUri) {
    vscode.window.showWarningMessage(vscode.l10n.t('Open a Verilog or SystemVerilog file first.'));
    return;
  }

  if (!isQiheSourceUri(targetUri)) {
    vscode.window.showWarningMessage(
      vscode.l10n.t('Qihe analysis is only available for Verilog files.'),
    );
    return;
  }

  if (!client) {
    await showLanguageServerErrorMessage(vscode.l10n.t('Vide language server is not running.'));
    return;
  }

  const workspaceFolder = vscode.workspace.getWorkspaceFolder(targetUri);
  const payload = {
    uri: targetUri.toString(),
    cwd: workspaceFolder?.uri.fsPath,
  };

  const target = workspaceFolder
    ? `workspace ${workspaceFolder.uri.fsPath}`
    : `file ${targetUri.fsPath}`;
  logQihe(`[INFO] Starting Qihe analysis for ${target}`);

  try {
    await client.sendRequest('workspace/executeCommand', {
      command: runQiheAnalysisRequest,
      arguments: [payload],
    });
  } catch (error) {
    const message = vscode.l10n.t('Failed to run Qihe analysis: {0}', (error as Error).message);
    log(`[ERROR] ${message}`);
    await showLanguageServerErrorMessage(message);
  }
}

function qiheAnalysisTargetUri(resource: unknown): vscode.Uri | undefined {
  if (resource instanceof vscode.Uri) {
    return resource;
  }
  return vscode.window.activeTextEditor?.document.uri;
}

function isQiheSourceUri(uri: vscode.Uri): boolean {
  if (uri.scheme !== 'file') {
    return false;
  }
  return ['.v', '.vh', '.sv', '.svh', '.svi'].includes(path.extname(uri.fsPath).toLowerCase());
}

function affectsServerLaunchConfiguration(event: vscode.ConfigurationChangeEvent): boolean {
  return (
    event.affectsConfiguration('vide.server.command') ||
    event.affectsConfiguration('vide.server.args') ||
    event.affectsConfiguration('vide.server.additionalArgs') ||
    event.affectsConfiguration('vide.server.cwd') ||
    event.affectsConfiguration('vide.trace.server')
  );
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(languageServerOutputChannelName);
  context.subscriptions.push(outputChannel);
  qiheOutputChannel = vscode.window.createOutputChannel(qiheOutputChannelName);
  context.subscriptions.push(qiheOutputChannel);
  videStatusController = new VideStatusController({
    createManifest: (rootUris) => createProjectConfigsFromRootUris(context, rootUris),
    profileDiagnostics: async () => {
      await vscode.commands.executeCommand(profileDiagnosticsCommand);
    },
    reloadProject: reloadWorkspace,
    restartServer: () => restartClient(context),
    showOutput,
    log,
  });
  context.subscriptions.push(videStatusController);
  qiheStatusBarItem = createQiheStatusBarItem();
  context.subscriptions.push(qiheStatusBarItem);
  updateServerStatus('stopped');

  log('[INFO] Vide extension activating...');
  log(`[INFO] Extension version: ${extensionBuildLabel(context)}`);
  log(`[INFO] Extension path: ${context.extensionPath}`);
  log(`[INFO] Platform: ${process.platform}-${process.arch}`);
  log(`[INFO] VS Code version: ${vscode.version}`);

  const showOutputRegistration = vscode.commands.registerCommand(showOutputCommand, () => {
    showOutput();
  });
  context.subscriptions.push(showOutputRegistration);

  const showQiheOutputRegistration = vscode.commands.registerCommand(showQiheOutputCommand, () => {
    showQiheOutput();
  });
  context.subscriptions.push(showQiheOutputRegistration);

  const restartCommandRegistration = vscode.commands.registerCommand(
    restartServerCommand,
    async () => {
      log('[INFO] Restart command triggered');
      await restartClient(context);
    },
  );
  context.subscriptions.push(restartCommandRegistration);

  const showVersionRegistration = vscode.commands.registerCommand(
    showServerVersionCommand,
    async () => {
      log('[INFO] Server version command triggered');
      await showServerVersion(context);
    },
  );
  context.subscriptions.push(showVersionRegistration);

  const runQiheRegistration = vscode.commands.registerCommand(
    runQiheAnalysisCommand,
    async (resource) => {
      await runQiheAnalysis(resource);
    },
  );
  context.subscriptions.push(runQiheRegistration);
  context.subscriptions.push(
    registerProfilingCommand(context, {
      resolveLaunch: () => resolveServerLaunch(context, readConfiguration()),
      createEnv: createServerEnv,
    }),
  );

  const reloadWorkspaceRegistration = vscode.commands.registerCommand(
    reloadWorkspaceCommand,
    async () => {
      await reloadWorkspace();
    },
  );
  context.subscriptions.push(reloadWorkspaceRegistration);

  const showStatusRegistration = vscode.commands.registerCommand(
    showStatusCommand,
    async () => {
      await videStatusController?.show();
    },
  );
  context.subscriptions.push(showStatusRegistration);
  registerDiagnosticActions(context);

  const configurationRegistration = vscode.workspace.onDidChangeConfiguration(
    async (event) => {
      if (!affectsServerLaunchConfiguration(event)) {
        return;
      }

      log('[INFO] Server launch configuration changed');
      const restartAction = vscode.l10n.t('Restart');
      const selection = await vscode.window.showInformationMessage(
        vscode.l10n.t(
          'Vide server configuration changed. Restart the language server to apply it.',
        ),
        restartAction,
      );
      if (selection === restartAction) {
        await restartClient(context);
      }
    },
  );
  context.subscriptions.push(configurationRegistration);

  await startClient(context);
  void promptForMissingProjectConfigs(context);

  log('[INFO] Vide extension activated');
}

export async function deactivate(): Promise<void> {
  clearQiheStatusHideTimer();
  for (const { resolve } of qiheProgressNotifications.values()) {
    resolve();
  }
  qiheProgressNotifications.clear();
  activeQiheTokens.clear();

  if (outputChannel) {
    log('[INFO] Vide extension deactivating...');
  }
  await stopClient();
  if (outputChannel) {
    log('[INFO] Vide extension deactivated');
  }
}
