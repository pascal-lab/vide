import * as fs from 'node:fs';

import * as vscode from 'vscode';
import {
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { getBundledServerPath, getPlatformFolder } from './platform';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;

function log(message: string): void {
  outputChannel?.appendLine(message);
}

function requireOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    throw new Error('Vizsla output channel has not been initialized.');
  }

  return outputChannel;
}

interface ServerConfiguration {
  command: string | undefined;
  args: string[];
  additionalArgs: string[];
  cwd: string | undefined;
  trace: 'off' | 'messages' | 'verbose';
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
  const config = vscode.workspace.getConfiguration('vizslaLsp');
  const command = config.get<string | null>('server.command');
  const args = asStringArray(config.get<unknown>('server.args'));
  const additionalArgs = asStringArray(config.get<unknown>('server.additionalArgs'));
  const cwd = config.get<string | null>('server.cwd');
  const trace = config.get<'off' | 'messages' | 'verbose'>('trace.server') ?? 'off';

  if (!args || !additionalArgs) {
    vscode.window.showErrorMessage('vizslaLsp server arguments settings must be arrays of strings.');
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

async function createClient(context: vscode.ExtensionContext): Promise<LanguageClient> {
  const channel = requireOutputChannel();
  log('[INFO] Creating language client...');

  const config = readConfiguration();
  const cwd = resolveWorkingDirectory(context, config.cwd);

  let serverCommand = config.command;
  if (!serverCommand) {
    serverCommand = getServerPath(context);
    if (!serverCommand) {
      const message =
        'Bundled Vizsla Language Server binary not found. Install the VSIX that matches your platform or configure "vizslaLsp.server.command".';
      log(`[ERROR] ${message}`);
      vscode.window.showErrorMessage(message);
      throw new Error(message);
    }
  } else {
    log(`[INFO] Using custom server command: ${serverCommand}`);
  }

  const serverArgs = [...config.args, ...config.additionalArgs];
  log(`[INFO] Server command: ${serverCommand}`);
  log(`[INFO] Server args: ${JSON.stringify(serverArgs)}`);
  log(`[INFO] Working directory: ${cwd}`);

  const commonEnv = {
    ...process.env,
    RUST_BACKTRACE: '1',
    RUST_LOG: 'info',
  };

  const serverOptions: ServerOptions = {
    run: {
      command: serverCommand,
      args: serverArgs,
      options: { cwd, env: commonEnv },
    },
    debug: {
      command: serverCommand,
      args: serverArgs,
      options: {
        cwd,
        env: {
          ...commonEnv,
          RUST_BACKTRACE: 'full',
          RUST_LOG: 'debug',
        },
      },
    },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'verilog' },
      { scheme: 'file', language: 'systemverilog' },
    ],
    synchronize: {
      configurationSection: ['vizslaLsp', 'vizsla'],
      fileEvents: [
        vscode.workspace.createFileSystemWatcher('**/*.v'),
        vscode.workspace.createFileSystemWatcher('**/*.vh'),
        vscode.workspace.createFileSystemWatcher('**/*.sv'),
        vscode.workspace.createFileSystemWatcher('**/*.svh'),
        vscode.workspace.createFileSystemWatcher('**/*.svi'),
      ],
    },
    outputChannel: channel,
    traceOutputChannel: channel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationOptions: {},
    ...(config.trace !== 'off' && { trace: config.trace }),
  };

  log('[INFO] Creating LanguageClient instance...');
  return new LanguageClient('vizslaLsp', 'Vizsla Language Server', serverOptions, clientOptions);
}

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  try {
    log('[INFO] Starting language server...');
    client = await createClient(context);
    await client.start();
    log('[INFO] Language server started successfully');
    vscode.window.showInformationMessage('Vizsla Language Server started');
  } catch (error) {
    log(`[ERROR] Failed to start language server: ${(error as Error).message}`);
    log(`[ERROR] ${(error as Error).stack}`);
    vscode.window.showErrorMessage(
      `Failed to start Vizsla Language Server: ${(error as Error).message}`,
    );
  }
}

async function stopClient(): Promise<void> {
  if (!client) {
    return;
  }

  log('[INFO] Stopping language server...');
  try {
    await client.stop();
    log('[INFO] Language server stopped');
  } catch (error) {
    log(`[ERROR] Error stopping language server: ${(error as Error).message}`);
  }
  client = undefined;
}

async function restartClient(context: vscode.ExtensionContext): Promise<void> {
  log('[INFO] Restarting language server...');
  await stopClient();
  await startClient(context);
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  outputChannel = vscode.window.createOutputChannel('Vizsla Language Server');
  context.subscriptions.push(outputChannel);

  log('[INFO] Vizsla extension activating...');
  log(`[INFO] Extension path: ${context.extensionPath}`);
  log(`[INFO] Platform: ${process.platform}-${process.arch}`);
  log(`[INFO] VS Code version: ${vscode.version}`);

  const restartCommand = vscode.commands.registerCommand('vizsla.restartServer', async () => {
    log('[INFO] Restart command triggered');
    await restartClient(context);
  });
  context.subscriptions.push(restartCommand);

  await startClient(context);

  log('[INFO] Vizsla extension activated');
}

export async function deactivate(): Promise<void> {
  if (outputChannel) {
    log('[INFO] Vizsla extension deactivating...');
  }
  await stopClient();
  if (outputChannel) {
    log('[INFO] Vizsla extension deactivated');
  }
}
