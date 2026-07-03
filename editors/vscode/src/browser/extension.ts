import * as vscode from "vscode";

import { registerDiagnosticActions } from "../diagnosticActions";
import { createVideCodeDocumentSelector } from "../common/documentSelector";
import {
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_SOURCE_FILE_GLOB,
  isProjectConfigFileName,
  isProjectSourceFileName,
} from "../projectConfigCommon";
import {
  reloadWorkspaceCommand,
  showOutputCommand,
  showStatusCommand,
  VideStatusController,
} from "../videStatus";
import type { ServerStatus } from "../status";
import { BrowserClientController } from "./clientController";
import { affectsBrowserClientConfiguration } from "./configuration";
import {
  createProjectConfigAtRoot,
  shouldRestartForWatchedUri,
} from "./workspaceSnapshot";

const restartServerCommand = "vide.restartServer";
const showServerVersionCommand = "vide.showServerVersion";
const runQiheAnalysisCommand = "vide.runQiheAnalysis";
const generateQiheOptionsCommand = "vide.generateQiheOptions";
const profileDiagnosticsCommand = "vide.profileDiagnostics";
const languageServerOutputChannelName = "Vide Language Server";

interface ExtensionBuildInfo {
  kind?: string;
  commitHash?: string;
  buildDate?: string;
  profileTrace?: boolean;
}

let outputChannel: vscode.OutputChannel | undefined;
let videStatusController: VideStatusController | undefined;
let clientController: BrowserClientController | undefined;

function log(message: string): void {
  outputChannel?.appendLine(message);
}

function requireOutputChannel(): vscode.OutputChannel {
  if (!outputChannel) {
    throw new Error(vscode.l10n.t("Vide output channel has not been initialized."));
  }
  return outputChannel;
}

function showOutput(): void {
  requireOutputChannel().show(true);
}

async function showLanguageServerErrorMessage(message: string): Promise<void> {
  const showOutputAction = vscode.l10n.t("Show Output");
  const selection = await vscode.window.showErrorMessage(
    message,
    showOutputAction,
  );
  if (selection === showOutputAction) {
    showOutput();
  }
}

function updateServerStatus(status: ServerStatus, detail?: string): void {
  videStatusController?.updateServerStatus(status, detail);
}

function extensionVersion(context: vscode.ExtensionContext): string {
  const packageJson = context.extension.packageJSON as { version?: unknown };
  return typeof packageJson.version === "string" && packageJson.version.length > 0
    ? packageJson.version
    : "unknown";
}

async function extensionBuildInfo(
  context: vscode.ExtensionContext,
): Promise<ExtensionBuildInfo | undefined> {
  try {
    const bytes = await vscode.workspace.fs.readFile(
      vscode.Uri.joinPath(context.extensionUri, "build-info.json"),
    );
    return JSON.parse(new TextDecoder("utf-8").decode(bytes)) as ExtensionBuildInfo;
  } catch {
    return undefined;
  }
}

async function extensionBuildLabel(
  context: vscode.ExtensionContext,
): Promise<string> {
  const version = extensionVersion(context);
  const buildInfo = await extensionBuildInfo(context);
  const details = [
    buildInfo?.kind,
    buildInfo?.commitHash,
    buildInfo?.buildDate,
  ].filter((part): part is string => typeof part === "string" && part.length > 0);
  return details.length > 0 ? `${version} (${details.join(", ")})` : version;
}

async function createProjectConfigsFromRootUris(
  rootUris: readonly string[],
): Promise<void> {
  const created: vscode.Uri[] = [];
  for (const rootUri of rootUris) {
    created.push(await createProjectConfigAtRoot(rootUri));
  }

  await clientController?.queueRestart("project manifest created");

  const action = vscode.l10n.t("Open Manifest");
  const selection = await vscode.window.showInformationMessage(
    created.length === 1
      ? vscode.l10n.t("Created {0}.", PROJECT_CONFIG_FILE_NAME)
      : vscode.l10n.t(
          "Created {0} in {1} workspace folders.",
          PROJECT_CONFIG_FILE_NAME,
          created.length,
        ),
    action,
  );
  if (selection === action && created[0]) {
    const document = await vscode.workspace.openTextDocument(created[0]);
    await vscode.window.showTextDocument(document);
  }
}

async function showServerVersion(
  context: vscode.ExtensionContext,
): Promise<void> {
  const buildLabel = await extensionBuildLabel(context);
  const serverInfo = clientController?.initializeServerInfo();
  const serverLabel = serverInfo
    ? `${serverInfo.name ?? "Vide"} ${serverInfo.version ?? ""}`.trim()
    : "unavailable";
  await vscode.window.showInformationMessage(
    vscode.l10n.t("Vide extension: {0}; server: {1}", buildLabel, serverLabel),
  );
}

async function showUnavailableInBrowser(feature: string): Promise<void> {
  await vscode.window.showInformationMessage(
    vscode.l10n.t("{0} is not available in vscode.dev yet.", feature),
  );
}

function registerWorkspaceWatchers(context: vscode.ExtensionContext): void {
  const sourceWatcher = vscode.workspace.createFileSystemWatcher(
    PROJECT_SOURCE_FILE_GLOB,
  );
  const manifestWatcher = vscode.workspace.createFileSystemWatcher(
    `**/${PROJECT_CONFIG_FILE_NAME}`,
  );

  const handleSourceEvent = (uri: vscode.Uri, label: string) => {
    if (!shouldRestartForWatchedUri(uri)) {
      return;
    }
    const openDocument = vscode.workspace.textDocuments.find(
      (document) => document.uri.toString() === uri.toString(),
    );
    if (
      openDocument &&
      isProjectSourceFileName(openDocument.fileName) &&
      !isProjectConfigFileName(openDocument.fileName)
    ) {
      return;
    }
    log(`[INFO] Workspace ${label}: ${uri.toString()}`);
    clientController?.scheduleRestart(`${label}: ${uri.toString()}`);
  };

  sourceWatcher.onDidCreate((uri) => handleSourceEvent(uri, "source created"));
  sourceWatcher.onDidDelete((uri) => handleSourceEvent(uri, "source deleted"));
  sourceWatcher.onDidChange((uri) => handleSourceEvent(uri, "source changed"));

  manifestWatcher.onDidCreate((uri) => handleSourceEvent(uri, "manifest created"));
  manifestWatcher.onDidDelete((uri) => handleSourceEvent(uri, "manifest deleted"));
  manifestWatcher.onDidChange((uri) => handleSourceEvent(uri, "manifest changed"));

  context.subscriptions.push(sourceWatcher, manifestWatcher);
}

export async function activate(
  context: vscode.ExtensionContext,
): Promise<void> {
  outputChannel = vscode.window.createOutputChannel(languageServerOutputChannelName);
  context.subscriptions.push(outputChannel);
  const buildInfo = await extensionBuildInfo(context);
  const profileTraceEnabled = buildInfo?.profileTrace === true;

  videStatusController = new VideStatusController({
    createManifest: (rootUris) => createProjectConfigsFromRootUris(rootUris),
    profileDiagnostics: profileTraceEnabled
      ? () => showUnavailableInBrowser("Diagnostics profiling")
      : undefined,
    reloadProject: async () => {
      await clientController?.queueRestart("reload project");
    },
    restartServer: async () => {
      await clientController?.queueRestart("restart command");
    },
    showOutput,
    log,
  });
  context.subscriptions.push(videStatusController);
  clientController = new BrowserClientController(context, {
    log,
    updateServerStatus,
    showLanguageServerErrorMessage,
    handleProjectStatusNotification: (params) => {
      videStatusController?.handleProjectNotification(params);
    },
  });
  updateServerStatus("stopped");

  log("[INFO] Vide browser extension activating...");
  log(`[INFO] Extension version: ${await extensionBuildLabel(context)}`);
  log(`[INFO] VS Code version: ${vscode.version}`);

  context.subscriptions.push(
    vscode.commands.registerCommand(showOutputCommand, () => showOutput()),
    vscode.commands.registerCommand(showStatusCommand, async () => {
      await videStatusController?.show();
    }),
    vscode.commands.registerCommand(restartServerCommand, async () => {
      await clientController?.queueRestart("restart command");
    }),
    vscode.commands.registerCommand(reloadWorkspaceCommand, async () => {
      await clientController?.queueRestart("reload project command");
    }),
    vscode.commands.registerCommand(showServerVersionCommand, async () => {
      await showServerVersion(context);
    }),
    vscode.commands.registerCommand(runQiheAnalysisCommand, async () => {
      await showUnavailableInBrowser("Qihe analysis");
    }),
    vscode.commands.registerCommand(generateQiheOptionsCommand, async () => {
      await showUnavailableInBrowser("Qihe options generation");
    }),
  );
  if (profileTraceEnabled) {
    context.subscriptions.push(
      vscode.commands.registerCommand(profileDiagnosticsCommand, async () => {
        await showUnavailableInBrowser("Diagnostics profiling");
      }),
    );
  }

  registerDiagnosticActions(context, createVideCodeDocumentSelector());
  registerWorkspaceWatchers(context);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (affectsBrowserClientConfiguration(event)) {
        clientController?.scheduleRestart("Vide configuration changed");
      }
    }),
  );

  await clientController.queueRestart("activation");
  log("[INFO] Vide browser extension activated.");
}

export async function deactivate(): Promise<void> {
  await clientController?.stop();
  clientController = undefined;
}
