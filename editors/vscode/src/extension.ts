import * as vscode from 'vscode';
import {
  LanguageClient,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { registerDiagnosticActions } from './diagnosticActions';
import { profileDiagnosticsCommand } from './profiling';
import {
  projectStatusNotification,
  reloadWorkspaceCommand,
  reloadWorkspaceRequest,
  showOutputCommand,
  showStatusCommand,
  VideStatusController,
} from './videStatus';
import type { ServerStatus } from './status';
import { createNodeClientOptions } from './common/clientOptions';
import { createProvideExpandedRenameEdits } from './common/renameMiddleware';
import {
  createServerEnv,
  readConfiguration,
  resolveServerLaunch,
} from './node/serverLaunch';
import {
  extensionBuildLabel,
  isProfileTraceEnabled,
} from './node/buildInfo';
import { registerProfilingIntegration } from './node/profilingIntegration';
import { showServerVersion } from './node/serverVersion';
import { QiheController } from './node/qihe';
import {
  createProjectConfigsFromRootUris,
  promptForMissingProjectConfigs,
  type ProjectConfigPromptActions,
} from './node/projectConfigPrompt';

let client: LanguageClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let videStatusController: VideStatusController | undefined;
let qiheController: QiheController | undefined;

const restartServerCommand = 'vide.restartServer';
const showServerVersionCommand = 'vide.showServerVersion';
// Output channel names are stable identifiers in the Output view.
const languageServerOutputChannelName = 'Vide Language Server';

function log(message: string): void {
  outputChannel?.appendLine(message);
}

function requireOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    throw new Error(vscode.l10n.t('Vide output channel has not been initialized.'));
  }

  return outputChannel;
}

function showOutput(): void {
  requireOutputChannel().show(true);
}

async function showLanguageServerErrorMessage(message: string): Promise<void> {
  const showOutputAction = vscode.l10n.t('Show Output');
  const selection = await vscode.window.showErrorMessage(message, showOutputAction);
  if (selection === showOutputAction) {
    showOutput();
  }
}

function updateServerStatus(status: ServerStatus, detail?: string): void {
  videStatusController?.updateServerStatus(status, detail);
}

function registerProjectStatusNotifications(languageClient: LanguageClient): void {
  languageClient.onNotification(projectStatusNotification, (params: unknown) => {
    videStatusController?.handleProjectNotification(params);
  });
}

async function createClient(context: vscode.ExtensionContext): Promise<LanguageClient> {
  const channel = requireOutputChannel();
  log('[INFO] Creating language client...');

  const config = readConfiguration();
  const launch = resolveServerLaunch(context, config, log);
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

  const clientOptions = createNodeClientOptions({
    outputChannel: channel,
    trace: config.trace,
    provideRenameEdits: createProvideExpandedRenameEdits(
      () => client,
      (message) => log(`[WARN] ${message}`),
    ),
  });

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
    qiheController?.registerNotifications(client);
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
  qiheController = new QiheController({
    getClient: () => client,
    logLanguageServer: log,
    showLanguageServerErrorMessage,
  });
  qiheController.register(context);
  const profileTraceEnabled = isProfileTraceEnabled(context);
  const projectConfigActions: ProjectConfigPromptActions = {
    hasClient: () => client !== undefined,
    restartClient,
    log,
  };
  videStatusController = new VideStatusController({
    createManifest: (rootUris) => createProjectConfigsFromRootUris(
      context,
      rootUris,
      projectConfigActions,
    ),
    profileDiagnostics: profileTraceEnabled
      ? async () => {
          await vscode.commands.executeCommand(profileDiagnosticsCommand);
        }
      : undefined,
    reloadProject: reloadWorkspace,
    restartServer: () => restartClient(context),
    showOutput,
    log,
  });
  context.subscriptions.push(videStatusController);
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
      await showServerVersion(context, { log, showLanguageServerErrorMessage });
    },
  );
  context.subscriptions.push(showVersionRegistration);

  const profilingRegistration = registerProfilingIntegration(context, {
    enabled: profileTraceEnabled,
    log,
  });
  if (profilingRegistration) {
    context.subscriptions.push(profilingRegistration);
  }

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
  void promptForMissingProjectConfigs(context, projectConfigActions);

  log('[INFO] Vide extension activated');
}

export async function deactivate(): Promise<void> {
  qiheController?.dispose();
  qiheController = undefined;

  if (outputChannel) {
    log('[INFO] Vide extension deactivating...');
  }
  await stopClient();
  if (outputChannel) {
    log('[INFO] Vide extension deactivated');
  }
}
