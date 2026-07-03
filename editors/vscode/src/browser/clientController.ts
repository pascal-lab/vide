import * as vscode from "vscode";

import type { ServerStatus } from "../status";
import { projectStatusNotification } from "../videStatus";
import { VideBrowserClient } from "./client";
import { buildBrowserWorkspaceSnapshot } from "./workspaceSnapshot";

type Logger = (message: string) => void;

export class BrowserClientController {
  private client: VideBrowserClient | undefined;
  private restartChain: Promise<void> = Promise.resolve();
  private workspaceRestartTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly options: {
      log: Logger;
      updateServerStatus: (status: ServerStatus, detail?: string) => void;
      showLanguageServerErrorMessage: (message: string) => Promise<void>;
      handleProjectStatusNotification: (params: unknown) => void;
    },
  ) {}

  getClient(): VideBrowserClient | undefined {
    return this.client;
  }

  hasClient(): boolean {
    return this.client !== undefined;
  }

  initializeServerInfo(): { name?: string; version?: string } | undefined {
    return this.client?.initializeServerInfo();
  }

  queueRestart(reason: string): Promise<void> {
    this.restartChain = this.restartChain
      .catch(() => undefined)
      .then(async () => {
        this.options.log(`[INFO] Restarting browser language client: ${reason}`);
        await this.stopClient();
        await this.startClient();
      });
    return this.restartChain;
  }

  scheduleRestart(reason: string): void {
    if (this.workspaceRestartTimer) {
      clearTimeout(this.workspaceRestartTimer);
    }
    this.workspaceRestartTimer = setTimeout(() => {
      this.workspaceRestartTimer = undefined;
      void this.queueRestart(reason);
    }, 250);
  }

  async stop(): Promise<void> {
    this.clearScheduledRestart();
    await this.stopClient();
  }

  private async startClient(): Promise<void> {
    this.options.updateServerStatus("starting");
    this.options.log("[INFO] Building browser workspace snapshot...");

    let startedClient: VideBrowserClient | undefined;
    try {
      const snapshot = await buildBrowserWorkspaceSnapshot(this.options.log);
      const browserClient = new VideBrowserClient(this.context, snapshot);
      startedClient = browserClient;
      this.client = browserClient;

      browserClient.onStatus = (status) => {
        if (!this.isActiveClient(browserClient)) {
          return;
        }
        this.options.updateServerStatus(status.ready ? "ready" : "error", status.detail);
      };
      browserClient.onServerCapabilities = () => undefined;
      browserClient.onLog = (message, level) => {
        if (!this.isActiveClient(browserClient)) {
          return;
        }
        this.options.log(`[${level.toUpperCase()}] ${message}`);
      };
      browserClient.onTrace = (entry) => {
        if (!this.isActiveClient(browserClient)) {
          return;
        }
        this.options.log(`[TRACE] ${entry.direction} ${entry.method} ${entry.detail}`);
      };

      browserClient.start();
      browserClient.onNotification(projectStatusNotification, (params) => {
        if (!this.isActiveClient(browserClient)) {
          return;
        }
        this.options.handleProjectStatusNotification(params);
      });
      this.options.log("[INFO] Browser language client booted.");
    } catch (error) {
      if (!startedClient || this.isActiveClient(startedClient)) {
        this.client = undefined;
      }
      const message =
        error instanceof Error
          ? error.message
          : "Failed to start the Vide browser extension.";
      this.options.log(`[ERROR] ${message}`);
      this.options.updateServerStatus("error", message);
      await this.options.showLanguageServerErrorMessage(
        vscode.l10n.t("Failed to start Vide Language Server: {0}", message),
      );
    }
  }

  private async stopClient(): Promise<void> {
    if (!this.client) {
      this.options.updateServerStatus("stopped");
      return;
    }

    this.options.updateServerStatus("stopping");
    this.client.dispose();
    this.client = undefined;
    this.options.updateServerStatus("stopped");
  }

  private isActiveClient(browserClient: VideBrowserClient): boolean {
    return this.client === browserClient;
  }

  private clearScheduledRestart(): void {
    if (!this.workspaceRestartTimer) {
      return;
    }
    clearTimeout(this.workspaceRestartTimer);
    this.workspaceRestartTimer = undefined;
  }
}
