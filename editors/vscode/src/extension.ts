import * as vscode from 'vscode';

import { registerDiagnosticActions } from './diagnosticActions';
import { profileDiagnosticsCommand } from './profiling';
import {
  reloadWorkspaceCommand,
  showOutputCommand,
  showStatusCommand,
  VideStatusController,
} from './videStatus';
import type { ServerStatus } from './status';
import {
  extensionBuildLabel,
  isProfileTraceEnabled,
} from './node/buildInfo';
import { registerProfilingIntegration } from './node/profilingIntegration';
import { showServerVersion } from './node/serverVersion';
import { QiheController } from './node/qihe';
import { NodeClientController } from './node/clientController';
import {
  createProjectConfigsFromRootUris,
  promptForMissingProjectConfigs,
  type ProjectConfigPromptActions,
} from './node/projectConfigPrompt';

let outputChannel: vscode.OutputChannel | undefined;
let videStatusController: VideStatusController | undefined;
let qiheController: QiheController | undefined;
let clientController: NodeClientController | undefined;

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
    getClient: () => clientController?.getClient(),
    logLanguageServer: log,
    showLanguageServerErrorMessage,
  });
  qiheController.register(context);
  clientController = new NodeClientController(context, {
    outputChannel: requireOutputChannel(),
    log,
    updateServerStatus,
    showLanguageServerErrorMessage,
    handleProjectStatusNotification: (params) => {
      videStatusController?.handleProjectNotification(params);
    },
    registerQiheNotifications: (languageClient) => {
      qiheController?.registerNotifications(languageClient);
    },
  });
  const profileTraceEnabled = isProfileTraceEnabled(context);
  const projectConfigActions: ProjectConfigPromptActions = {
    hasClient: () => clientController?.hasClient() ?? false,
    restartClient: async () => {
      await clientController?.restart();
    },
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
    reloadProject: async () => {
      await clientController?.reloadWorkspace();
    },
    restartServer: async () => {
      await clientController?.restart();
    },
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
      await clientController?.restart();
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
      await clientController?.reloadWorkspace();
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
        await clientController?.restart();
      }
    },
  );
  context.subscriptions.push(configurationRegistration);

  await clientController.start();
  void promptForMissingProjectConfigs(context, projectConfigActions);

  log('[INFO] Vide extension activated');
}

export async function deactivate(): Promise<void> {
  qiheController?.dispose();
  qiheController = undefined;

  if (outputChannel) {
    log('[INFO] Vide extension deactivating...');
  }
  await clientController?.stop();
  clientController = undefined;
  if (outputChannel) {
    log('[INFO] Vide extension deactivated');
  }
}
