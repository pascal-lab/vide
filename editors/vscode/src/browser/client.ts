import * as vscode from "vscode";
import {
  BaseLanguageClient,
  BrowserMessageReader,
  BrowserMessageWriter,
  CloseAction,
  ErrorAction,
  type LanguageClientOptions,
  type MessageTransports,
} from "vscode-languageclient/browser";

import { videInitializationOptions } from "../../../../packages/vide-extension-shared/src/config/initialization-options";
import type {
  LspTraceEntry,
  WorkerRequest,
  WorkerResponse,
  WorkerStatus,
} from "../../../../packages/vide-extension-shared/src/browser/types";
import {
  BROWSER_WORKSPACE_FOLDER_NAME,
  type BrowserWorkspaceSnapshot,
} from "./workspaceSnapshot";

const CLIENT_DISPOSED_MESSAGE = "Vide browser client has been disposed.";

export class VideBrowserClient {
  private readonly worker: Worker;
  private languageClient?: VideLanguageClient;
  private workerReadyStatus?: WorkerStatus;
  private disposed = false;

  onStatus: (status: WorkerStatus) => void = () => undefined;
  onServerCapabilities: (capabilities: unknown) => void = () => undefined;
  onTrace: (entry: LspTraceEntry) => void = () => undefined;
  onLog: (message: string, level: "info" | "warn" | "error") => void =
    () => undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly snapshot: BrowserWorkspaceSnapshot,
  ) {
    const workerUri = vscode.Uri.joinPath(
      context.extensionUri,
      "dist",
      "browser",
      "vide-lsp.worker.js",
    );
    this.worker = new Worker(workerUri.toString(true));
    this.worker.addEventListener("message", (event: MessageEvent<WorkerResponse>) => {
      this.handleMessage(event.data);
    });
  }

  start(): void {
    const channel = new MessageChannel();
    this.languageClient = new VideLanguageClient(this.clientOptions(), {
      reader: new BrowserMessageReader(channel.port1),
      writer: new BrowserMessageWriter(channel.port1),
    });
    this.post(
      {
        kind: "boot",
        wasmBaseUrl: vscode.Uri.joinPath(
          this.context.extensionUri,
          "dist",
          "browser",
          "wasm",
        ).toString(),
        rootUri: this.snapshot.rootUri,
        workspaceRootUris: this.snapshot.workspaceRootUris,
        workspaceFiles: this.snapshot.workspaceFiles,
        lspPort: channel.port2,
      },
      [channel.port2],
    );
  }

  onNotification(
    method: string,
    handler: (params: unknown) => void,
  ): vscode.Disposable {
    this.requireLanguageClient().onNotification(method, handler);
    return new vscode.Disposable(() => undefined);
  }

  request(method: string, params?: unknown): Promise<unknown> {
    if (this.disposed) {
      return Promise.reject(new Error(CLIENT_DISPOSED_MESSAGE));
    }
    return this.requireLanguageClient().sendRequest(method, params);
  }

  initializeServerInfo():
    | { name?: string; version?: string }
    | undefined {
    return this.languageClient?.initializeResult?.serverInfo;
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.post({ kind: "stop" });
    this.disposed = true;
    void this.languageClient?.dispose(500).catch(() => undefined);
    this.worker.terminate();
  }

  private post(message: WorkerRequest, transfer: Transferable[] = []): void {
    if (this.disposed) {
      return;
    }
    this.worker.postMessage(message, transfer);
  }

  private requireLanguageClient(): VideLanguageClient {
    if (!this.languageClient || this.disposed) {
      throw new Error(CLIENT_DISPOSED_MESSAGE);
    }
    return this.languageClient;
  }

  private clientOptions(): LanguageClientOptions {
    return {
      documentSelector: [
        { language: "verilog" },
        { language: "systemverilog" },
      ],
      workspaceFolder: {
        index: 0,
        name: BROWSER_WORKSPACE_FOLDER_NAME,
        uri: vscode.Uri.parse(this.snapshot.rootUri),
      },
      initializationOptions: videInitializationOptions(
        vscode.workspace.getConfiguration("vide"),
      ),
      diagnosticPullOptions: {
        onChange: false,
        onSave: false,
        onTabs: false,
      },
      errorHandler: {
        error: (error) => {
          this.onLog(error.message, "error");
          return { action: ErrorAction.Shutdown };
        },
        closed: () => ({ action: CloseAction.DoNotRestart }),
      },
      middleware: {
        handleDiagnostics: (uri, diagnostics, next) => {
          next(uri, diagnostics);
        },
        workspace: {
          configuration: () => [],
        },
      },
    };
  }

  private handleMessage(message: WorkerResponse): void {
    switch (message.kind) {
      case "status":
        if (message.status.ready) {
          this.workerReadyStatus = message.status;
          void this.startLanguageClient();
        } else {
          this.onStatus(message.status);
        }
        break;
      case "trace":
        this.onTrace(message.entry);
        break;
      case "log":
        this.onLog(message.message, message.level);
        break;
    }
  }

  private async startLanguageClient(): Promise<void> {
    const languageClient = this.languageClient;
    const workerReadyStatus = this.workerReadyStatus;
    if (
      !languageClient ||
      !workerReadyStatus ||
      this.disposed ||
      languageClient.isRunning()
    ) {
      return;
    }

    try {
      await languageClient.start();
      this.onServerCapabilities(
        languageClient.initializeResult?.capabilities ?? null,
      );
      this.onStatus(workerReadyStatus);
    } catch (error) {
      this.onStatus({
        engine: "unavailable",
        ready: false,
        detail:
          error instanceof Error
            ? error.message
            : "Vide language client failed to start.",
      });
    }
  }
}

class VideLanguageClient extends BaseLanguageClient {
  constructor(
    clientOptions: LanguageClientOptions,
    private readonly messageTransports: MessageTransports,
  ) {
    super("vide", "Vide", clientOptions);
  }

  protected createMessageTransports(): Promise<MessageTransports> {
    return Promise.resolve(this.messageTransports);
  }
}
