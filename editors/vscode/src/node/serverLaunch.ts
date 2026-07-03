import * as fs from 'node:fs';

import * as vscode from 'vscode';

import { getBundledServerPath, getPlatformFolder } from '../platform';

type Logger = (message: string) => void;

export interface ServerConfiguration {
  command: string | undefined;
  args: string[];
  additionalArgs: string[];
  cwd: string | undefined;
  trace: 'off' | 'messages' | 'verbose';
}

export interface ServerLaunch {
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

export function readConfiguration(): ServerConfiguration {
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

export function getServerPath(
  context: vscode.ExtensionContext,
  log: Logger,
): string | undefined {
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

export function resolveWorkingDirectory(
  context: vscode.ExtensionContext,
  configuredCwd: string | undefined,
  log: Logger,
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

export function resolveServerLaunch(
  context: vscode.ExtensionContext,
  config: ServerConfiguration,
  log: Logger,
): ServerLaunch {
  const cwd = resolveWorkingDirectory(context, config.cwd, log);

  let serverCommand = config.command;
  if (!serverCommand) {
    serverCommand = getServerPath(context, log);
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

export function createServerEnv(
  logLevel: 'info' | 'debug' = 'info',
  backtrace: '1' | 'full' = '1',
): NodeJS.ProcessEnv {
  return {
    ...process.env,
    RUST_BACKTRACE: backtrace,
    RUST_LOG: logLevel,
  };
}
