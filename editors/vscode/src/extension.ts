import * as fs from 'node:fs';
import * as path from 'node:path';

import * as vscode from 'vscode';
import {
  LanguageClient,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { registerDiagnosticActions } from './diagnosticActions';
import { profileDiagnosticsCommand } from './profiling';
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
  videStatusController = new VideStatusController({
    createManifest: (rootUris) => createProjectConfigsFromRootUris(context, rootUris),
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
  void promptForMissingProjectConfigs(context);

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
