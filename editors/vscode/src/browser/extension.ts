import * as vscode from "vscode";

import { registerDiagnosticActions } from "../diagnosticActions";
import {
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_SOURCE_FILE_GLOB,
  isProjectConfigFileName,
  isProjectSourceFileName,
} from "../projectConfigCommon";
import {
  projectStatusNotification,
  reloadWorkspaceCommand,
  showOutputCommand,
  showStatusCommand,
  VideStatusController,
} from "../videStatus";
import type { ServerStatus } from "../status";
import { VideBrowserClient } from "./client";
import {
  buildBrowserWorkspaceSnapshot,
  createProjectConfigAtRoot,
  shouldRestartForWatchedUri,
} from "./workspaceSnapshot";

const restartServerCommand = "vide.restartServer";
const showServerVersionCommand = "vide.showServerVersion";
const runQiheAnalysisCommand = "vide.runQiheAnalysis";
const profileDiagnosticsCommand = "vide.profileDiagnostics";
const languageServerOutputChannelName = "Vide Language Server";

interface ExtensionBuildInfo {
  kind?: string;
  commitHash?: string;
  buildDate?: string;
}

let client: VideBrowserClient | undefined;
let outputChannel: vscode.OutputChannel | undefined;
let videStatusController: VideStatusController | undefined;
let restartChain: Promise<void> = Promise.resolve();
let workspaceRestartTimer: ReturnType<typeof setTimeout> | undefined;

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

async function startClient(context: vscode.ExtensionContext): Promise<void> {
  updateServerStatus("starting");
  log("[INFO] Building browser workspace snapshot...");

  try {
    const snapshot = await buildBrowserWorkspaceSnapshot(log);
    const browserClient = new VideBrowserClient(context, snapshot);
    client = browserClient;

    browserClient.onStatus = (status) => {
      if (client !== browserClient) {
        return;
      }
      updateServerStatus(status.ready ? "ready" : "error", status.detail);
    };
    browserClient.onServerCapabilities = () => undefined;
    browserClient.onLog = (message, level) => {
      if (client !== browserClient) {
        return;
      }
      log(`[${level.toUpperCase()}] ${message}`);
    };
    browserClient.onTrace = (entry) => {
      if (client !== browserClient) {
        return;
      }
      log(`[TRACE] ${entry.direction} ${entry.method} ${entry.detail}`);
    };

    browserClient.start();
    browserClient.onNotification(projectStatusNotification, (params) => {
      if (client !== browserClient) {
        return;
      }
      videStatusController?.handleProjectNotification(params);
    });
    log("[INFO] Browser language client booted.");
  } catch (error) {
    client = undefined;
    const message =
      error instanceof Error
        ? error.message
        : "Failed to start the Vide browser extension.";
    log(`[ERROR] ${message}`);
    updateServerStatus("error", message);
    await showLanguageServerErrorMessage(
      vscode.l10n.t("Failed to start Vide Language Server: {0}", message),
    );
  }
}

async function stopClient(): Promise<void> {
  if (!client) {
    updateServerStatus("stopped");
    return;
  }

  updateServerStatus("stopping");
  client.dispose();
  client = undefined;
  updateServerStatus("stopped");
}

function queueRestart(
  context: vscode.ExtensionContext,
  reason: string,
): Promise<void> {
  restartChain = restartChain
    .catch(() => undefined)
    .then(async () => {
      log(`[INFO] Restarting browser language client: ${reason}`);
      await stopClient();
      await startClient(context);
    });
  return restartChain;
}

function scheduleWorkspaceRestart(
  context: vscode.ExtensionContext,
  reason: string,
): void {
  if (workspaceRestartTimer) {
    clearTimeout(workspaceRestartTimer);
  }
  workspaceRestartTimer = setTimeout(() => {
    workspaceRestartTimer = undefined;
    void queueRestart(context, reason);
  }, 250);
}

async function createProjectConfigsFromRootUris(
  context: vscode.ExtensionContext,
  rootUris: readonly string[],
): Promise<void> {
  const created: vscode.Uri[] = [];
  for (const rootUri of rootUris) {
    created.push(await createProjectConfigAtRoot(rootUri));
  }

  await queueRestart(context, "project manifest created");

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
  const serverInfo = client?.initializeServerInfo();
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

function registerWorkspaceWatchers(
  context: vscode.ExtensionContext,
): void {
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
    scheduleWorkspaceRestart(context, `${label}: ${uri.toString()}`);
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

  videStatusController = new VideStatusController({
    createManifest: (rootUris) => createProjectConfigsFromRootUris(context, rootUris),
    profileDiagnostics: () => showUnavailableInBrowser("Diagnostics profiling"),
    reloadProject: () => queueRestart(context, "reload project"),
    restartServer: () => queueRestart(context, "restart command"),
    showOutput,
    log,
  });
  context.subscriptions.push(videStatusController);
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
      await queueRestart(context, "restart command");
    }),
    vscode.commands.registerCommand(reloadWorkspaceCommand, async () => {
      await queueRestart(context, "reload project command");
    }),
    vscode.commands.registerCommand(showServerVersionCommand, async () => {
      await showServerVersion(context);
    }),
    vscode.commands.registerCommand(runQiheAnalysisCommand, async () => {
      await showUnavailableInBrowser("Qihe analysis");
    }),
    vscode.commands.registerCommand(profileDiagnosticsCommand, async () => {
      await showUnavailableInBrowser("Diagnostics profiling");
    }),
  );

  registerDiagnosticActions(context);
  registerWorkspaceWatchers(context);

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("vide")) {
        scheduleWorkspaceRestart(context, "Vide configuration changed");
      }
    }),
  );

  await queueRestart(context, "activation");
  log("[INFO] Vide browser extension activated.");
}

export async function deactivate(): Promise<void> {
  if (workspaceRestartTimer) {
    clearTimeout(workspaceRestartTimer);
    workspaceRestartTimer = undefined;
  }
  await stopClient();
}
