import { execFile } from 'node:child_process';
import * as fs from 'node:fs';
import { promisify } from 'node:util';

import * as vscode from 'vscode';
import {
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { getBundledServerPath, getPlatformFolder } from './platform';
import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_FILE_NAME,
  getProjectConfigPath,
} from './projectConfig';
import { getServerStatusPresentation, type ServerStatus } from './status';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let statusBarItem: vscode.StatusBarItem | undefined;
let qiheStatusBarItem: vscode.StatusBarItem | undefined;

const execFileAsync = promisify(execFile);
const showOutputCommand = 'vizsla.showOutput';
const restartServerCommand = 'vizsla.restartServer';
const showServerVersionCommand = 'vizsla.showServerVersion';
const runQiheAnalysisCommand = 'vizsla.runQiheAnalysis';
const runQiheAnalysisRequest = 'vizsla.server.runQiheAnalysis';
const qiheStatusNotification = 'vizsla/qiheStatus';
const qiheAnalysisIcon = '$(beaker)';
const versionTimeoutMs = 5000;

const activeQiheTokens = new Set<string>();
const qiheProgressNotifications = new Map<string, { resolve: () => void }>();
let qiheStatusHideTimer: NodeJS.Timeout | undefined;

function log(message: string): void {
  outputChannel?.appendLine(message);
}

function requireOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    throw new Error('Vizsla output channel has not been initialized.');
  }

  return outputChannel;
}

function showOutput(): void {
  requireOutputChannel().show(true);
}

function updateServerStatus(status: ServerStatus, detail?: string): void {
  if (!statusBarItem) {
    return;
  }

  const presentation = getServerStatusPresentation(status, detail);
  statusBarItem.text = presentation.text;
  statusBarItem.tooltip = `${presentation.tooltip}\n\nClick to show output.`;
  statusBarItem.command = showOutputCommand;
  statusBarItem.color = presentation.color
    ? new vscode.ThemeColor(presentation.color)
    : undefined;
  statusBarItem.backgroundColor = presentation.backgroundColor
    ? new vscode.ThemeColor(presentation.backgroundColor)
    : undefined;
  statusBarItem.show();
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
): void {
  if (!qiheStatusBarItem) {
    return;
  }

  clearQiheStatusHideTimer();
  qiheStatusBarItem.text = `${qiheAnalysisIcon} Qihe`;
  qiheStatusBarItem.tooltip = tooltip;
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
  item.name = 'Vizsla Qihe';
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
      title: 'Running Qihe analysis',
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
          updateQiheStatus(message ?? 'Qihe analysis is running');
          startQiheNotification(token, message);
          break;
        case 'end':
          activeQiheTokens.delete(token);
          finishQiheNotification(token);
          if (activeQiheTokens.size === 0) {
            updateQiheStatus(message ?? 'Qihe analysis finished', 4000);
          }
          break;
        case 'failed':
          activeQiheTokens.delete(token);
          finishQiheNotification(token);
          if (activeQiheTokens.size === 0) {
            updateQiheStatus(message ?? 'Qihe analysis failed', 6000);
          }
          break;
        default:
          break;
      }
    },
  );
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
  const config = vscode.workspace.getConfiguration('vizsla');
  const command = config.get<string | null>('server.command');
  const args = asStringArray(config.get<unknown>('server.args'));
  const additionalArgs = asStringArray(config.get<unknown>('server.additionalArgs'));
  const cwd = config.get<string | null>('server.cwd');
  const trace = config.get<'off' | 'messages' | 'verbose'>('trace.server') ?? 'off';

  if (!args || !additionalArgs) {
    vscode.window.showErrorMessage('vizsla server arguments settings must be arrays of strings.');
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
      const message =
        'Bundled Vizsla Language Server binary not found. Install the VSIX that matches your platform or configure "vizsla.server.command".';
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

async function createMissingProjectConfigs(): Promise<void> {
  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  const createdConfigs: vscode.Uri[] = [];

  for (const folder of workspaceFolders) {
    if (folder.uri.scheme !== 'file') {
      log(
        `[WARN] Skipping project config creation for non-file workspace: ${folder.uri.toString()}`,
      );
      continue;
    }

    const configPath = getProjectConfigPath(folder.uri.fsPath);
    if (fs.existsSync(configPath)) {
      continue;
    }

    try {
      await fs.promises.writeFile(configPath, DEFAULT_PROJECT_CONFIG_TEXT, {
        encoding: 'utf8',
        flag: 'wx',
      });
      createdConfigs.push(vscode.Uri.file(configPath));
      log(`[INFO] Created default project config: ${configPath}`);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'EEXIST') {
        continue;
      }

      const message = `Failed to create ${PROJECT_CONFIG_FILE_NAME} in ${folder.name}: ${(error as Error).message}`;
      log(`[WARN] ${message}`);
      void vscode.window.showWarningMessage(message);
    }
  }

  if (createdConfigs.length === 0) {
    return;
  }

  const message =
    createdConfigs.length === 1
      ? `No ${PROJECT_CONFIG_FILE_NAME} found. Created a syntax-only project config.`
      : `No ${PROJECT_CONFIG_FILE_NAME} found in ${createdConfigs.length} workspace folders. Created syntax-only project configs.`;
  const openConfigAction = createdConfigs.length === 1 ? 'Open Config' : 'Open First Config';

  void vscode.window.showInformationMessage(message, openConfigAction).then(async (selection) => {
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
      configurationSection: ['vizsla'],
    },
    outputChannel: channel,
    traceOutputChannel: channel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationOptions: {},
    ...(config.trace !== 'off' && { trace: config.trace }),
  };

  log('[INFO] Creating LanguageClient instance...');
  return new LanguageClient('vizsla', 'Vizsla Language Server', serverOptions, clientOptions);
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  try {
    updateServerStatus('starting');
    log('[INFO] Starting language server...');
    client = await createClient(context);
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
    vscode.window.showErrorMessage(
      `Failed to start Vizsla Language Server: ${message}`,
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
    const output = `${stdout}${stderr}`.trim() || 'No version output';
    const firstLine = output.split(/\r?\n/, 1)[0] ?? output;
    log(`[INFO] Server version output:\n${output}`);
    vscode.window.showInformationMessage(`Vizsla server: ${firstLine}`);
  } catch (error) {
    const message = `Failed to query Vizsla server version: ${(error as Error).message}`;
    log(`[ERROR] ${message}`);
    const selection = await vscode.window.showErrorMessage(message, 'Show Output');
    if (selection === 'Show Output') {
      showOutput();
    }
  }
}

async function runQiheAnalysis(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage('Open a Verilog or SystemVerilog file first.');
    return;
  }

  if (!['verilog', 'systemverilog'].includes(editor.document.languageId)) {
    vscode.window.showWarningMessage('Qihe analysis is only available for Verilog files.');
    return;
  }

  if (!client) {
    vscode.window.showErrorMessage('Vizsla language server is not running.');
    return;
  }

  const workspaceFolder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
  const payload = {
    uri: editor.document.uri.toString(),
    cwd: workspaceFolder?.uri.fsPath,
  };

  log(`[INFO] Running Qihe analysis: ${JSON.stringify(payload)}`);

  try {
    await client.sendRequest('workspace/executeCommand', {
      command: runQiheAnalysisRequest,
      arguments: [payload],
    });
  } catch (error) {
    const message = `Failed to run Qihe analysis: ${(error as Error).message}`;
    log(`[ERROR] ${message}`);
    vscode.window.showErrorMessage(message);
  }
}

function affectsServerLaunchConfiguration(event: vscode.ConfigurationChangeEvent): boolean {
  return (
    event.affectsConfiguration('vizsla.server.command') ||
    event.affectsConfiguration('vizsla.server.args') ||
    event.affectsConfiguration('vizsla.server.additionalArgs') ||
    event.affectsConfiguration('vizsla.server.cwd') ||
    event.affectsConfiguration('vizsla.trace.server')
  );
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Vizsla Language Server');
  context.subscriptions.push(outputChannel);
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
  context.subscriptions.push(statusBarItem);
  qiheStatusBarItem = createQiheStatusBarItem();
  context.subscriptions.push(qiheStatusBarItem);
  updateServerStatus('stopped');

  log('[INFO] Vizsla extension activating...');
  log(`[INFO] Extension path: ${context.extensionPath}`);
  log(`[INFO] Platform: ${process.platform}-${process.arch}`);
  log(`[INFO] VS Code version: ${vscode.version}`);

  const showOutputRegistration = vscode.commands.registerCommand(showOutputCommand, () => {
    showOutput();
  });
  context.subscriptions.push(showOutputRegistration);

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
    async () => {
      await runQiheAnalysis();
    },
  );
  context.subscriptions.push(runQiheRegistration);

  const configurationRegistration = vscode.workspace.onDidChangeConfiguration(
    async (event) => {
      if (!affectsServerLaunchConfiguration(event)) {
        return;
      }

      log('[INFO] Server launch configuration changed');
      const selection = await vscode.window.showInformationMessage(
        'Vizsla server configuration changed. Restart the language server to apply it.',
        'Restart',
      );
      if (selection === 'Restart') {
        await restartClient(context);
      }
    },
  );
  context.subscriptions.push(configurationRegistration);

  await createMissingProjectConfigs();
  await startClient(context);

  log('[INFO] Vizsla extension activated');
}

export async function deactivate(): Promise<void> {
  clearQiheStatusHideTimer();
  for (const { resolve } of qiheProgressNotifications.values()) {
    resolve();
  }
  qiheProgressNotifications.clear();
  activeQiheTokens.clear();

  if (outputChannel) {
    log('[INFO] Vizsla extension deactivating...');
  }
  await stopClient();
  if (outputChannel) {
    log('[INFO] Vizsla extension deactivated');
  }
}
