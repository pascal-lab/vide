import * as path from 'node:path';

import * as vscode from 'vscode';
import type { LanguageClient } from 'vscode-languageclient/node';

import { registerQiheOptionsCommand } from '../qiheOptions';

type Logger = (message: string) => void;

const showQiheOutputCommand = 'vide.showQiheOutput';
const runQiheAnalysisCommand = 'vide.runQiheAnalysis';
const runQiheAnalysisRequest = 'vide.server.runQiheAnalysis';
const qiheStatusNotification = 'vide/qiheStatus';
const qiheLogNotification = 'vide/qiheLog';
const qiheAnalysisIcon = '$(beaker)';
const qiheOutputChannelName = 'Vide Qihe';

export class QiheController implements vscode.Disposable {
  private readonly outputChannel = vscode.window.createOutputChannel(qiheOutputChannelName);
  private readonly statusBarItem = createQiheStatusBarItem();
  private readonly activeTokens = new Set<string>();
  private readonly progressNotifications = new Map<string, { resolve: () => void }>();
  private statusHideTimer: NodeJS.Timeout | undefined;
  private disposed = false;

  constructor(
    private readonly options: {
      getClient: () => LanguageClient | undefined;
      logLanguageServer: Logger;
      showLanguageServerErrorMessage: (message: string) => Promise<void>;
    },
  ) {}

  register(context: vscode.ExtensionContext): void {
    context.subscriptions.push(this);
    context.subscriptions.push(
      vscode.commands.registerCommand(showQiheOutputCommand, () => {
        this.showOutput();
      }),
    );
    context.subscriptions.push(
      vscode.commands.registerCommand(runQiheAnalysisCommand, async (resource) => {
        await this.runAnalysis(resource);
      }),
    );
    context.subscriptions.push(
      registerQiheOptionsCommand({
        logQihe: (message) => this.log(message),
        showQiheErrorMessage: (message) => this.showErrorMessage(message),
      }),
    );
  }

  registerNotifications(languageClient: LanguageClient): void {
    languageClient.onNotification(
      qiheLogNotification,
      (params: { token?: unknown; message?: unknown }) => {
        const message =
          typeof params.message === 'string' ? params.message : undefined;

        if (!message) {
          return;
        }

        this.log(message);
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
            this.activeTokens.add(token);
            this.updateStatus(message ?? vscode.l10n.t('Qihe analysis is running'));
            this.startNotification(token, message);
            break;
          case 'end':
            this.activeTokens.delete(token);
            this.finishNotification(token);
            if (this.activeTokens.size === 0) {
              this.updateStatus(message ?? vscode.l10n.t('Qihe analysis finished'), 4000);
            }
            break;
          case 'failed':
            this.activeTokens.delete(token);
            this.finishNotification(token);
            if (this.activeTokens.size === 0) {
              const failureMessage = message ?? vscode.l10n.t('Qihe analysis failed');
              this.updateStatus(failureMessage, 6000, showQiheOutputCommand);
              void this.showErrorMessage(failureMessage);
            }
            break;
          default:
            break;
        }
      },
    );
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }

    this.disposed = true;
    this.clearStatusHideTimer();
    for (const { resolve } of this.progressNotifications.values()) {
      resolve();
    }
    this.progressNotifications.clear();
    this.activeTokens.clear();
    this.statusBarItem.dispose();
    this.outputChannel.dispose();
  }

  private log(message: string): void {
    this.outputChannel.appendLine(message);
  }

  private showOutput(): void {
    this.outputChannel.show(true);
  }

  private async showErrorMessage(message: string): Promise<void> {
    const showOutputAction = vscode.l10n.t('Show Qihe Output');
    const selection = await vscode.window.showErrorMessage(message, showOutputAction);
    if (selection === showOutputAction) {
      this.showOutput();
    }
  }

  private updateStatus(
    tooltip: string,
    hideAfterMs?: number,
    command: string | vscode.Command = runQiheAnalysisCommand,
  ): void {
    this.clearStatusHideTimer();
    this.statusBarItem.text = `${qiheAnalysisIcon} Qihe`;
    this.statusBarItem.tooltip = tooltip;
    this.statusBarItem.command = command;
    this.statusBarItem.show();

    if (!hideAfterMs) {
      return;
    }

    this.statusHideTimer = setTimeout(() => {
      this.statusBarItem.hide();
      this.statusHideTimer = undefined;
    }, hideAfterMs);
  }

  private clearStatusHideTimer(): void {
    if (!this.statusHideTimer) {
      return;
    }

    clearTimeout(this.statusHideTimer);
    this.statusHideTimer = undefined;
  }

  private startNotification(token: string, message?: string): void {
    if (this.progressNotifications.has(token)) {
      return;
    }

    let resolveProgress = () => {};
    const progressPromise = new Promise<void>((resolve) => {
      resolveProgress = resolve;
    });

    this.progressNotifications.set(token, { resolve: resolveProgress });

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

  private finishNotification(token: string): void {
    const entry = this.progressNotifications.get(token);
    if (!entry) {
      return;
    }

    this.progressNotifications.delete(token);
    entry.resolve();
  }

  private async runAnalysis(resource: unknown): Promise<void> {
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

    const languageClient = this.options.getClient();
    if (!languageClient) {
      await this.options.showLanguageServerErrorMessage(
        vscode.l10n.t('Vide language server is not running.'),
      );
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
    this.log(`[INFO] Starting Qihe analysis for ${target}`);

    try {
      await languageClient.sendRequest('workspace/executeCommand', {
        command: runQiheAnalysisRequest,
        arguments: [payload],
      });
    } catch (error) {
      const message = vscode.l10n.t('Failed to run Qihe analysis: {0}', (error as Error).message);
      this.options.logLanguageServer(`[ERROR] ${message}`);
      await this.options.showLanguageServerErrorMessage(message);
    }
  }
}

function createQiheStatusBarItem(): vscode.StatusBarItem {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.name = vscode.l10n.t('Qihe Analysis Status');
  item.command = runQiheAnalysisCommand;
  item.hide();
  return item;
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
