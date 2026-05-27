import "vscode/localExtensionHost";
import * as vscode from "vscode";
import VideWorker from "../workers/vide-lsp.worker?worker&inline";
import type { LspTraceEntry, WorkerRequest, WorkerResponse, WorkerStatus, WorkerWorkspaceFile } from "../types";
import { browserInitializationOptions } from "../workers/lsp-browser-config";
import {
  BaseLanguageClient,
  CloseAction,
  ErrorAction,
  type LanguageClientOptions,
  type MessageTransports,
} from "vscode-languageclient/browser.js";
import { BrowserMessageReader, BrowserMessageWriter } from "vscode-languageserver-protocol/browser.js";

const CLIENT_DISPOSED_MESSAGE = "Vide LSP client has been disposed.";

export function isClientDisposedError(error: unknown): boolean {
  return error instanceof Error && error.message === CLIENT_DISPOSED_MESSAGE;
}

export class VideBrowserClient {
  private readonly worker = new VideWorker();
  private readonly wasmBaseUrl: string;
  private readonly rootUri: string;
  private languageClient?: VideLanguageClient;
  private workerReadyStatus?: WorkerStatus;
  private disposed = false;

  onStatus: (status: WorkerStatus) => void = () => undefined;
  onServerCapabilities: (capabilities: unknown) => void = () => undefined;
  onTrace: (entry: LspTraceEntry) => void = () => undefined;
  onLog: (message: string, level: "info" | "warn" | "error") => void = () => undefined;

  constructor(wasmBaseUrl = "/wasm/", rootUri = "file:///workspace") {
    this.wasmBaseUrl = new URL(wasmBaseUrl, window.location.href).href;
    this.rootUri = rootUri;
    this.worker.addEventListener("message", (event: MessageEvent<WorkerResponse>) => {
      this.handleMessage(event.data);
    });
  }

  start(workspaceFiles: WorkerWorkspaceFile[]): void {
    const channel = new MessageChannel();
    this.languageClient = new VideLanguageClient(this.clientOptions(), {
      reader: new BrowserMessageReader(channel.port1),
      writer: new BrowserMessageWriter(channel.port1),
    });
    this.post({ kind: "boot", wasmBaseUrl: this.wasmBaseUrl, rootUri: this.rootUri, workspaceFiles, lspPort: channel.port2 }, [
      channel.port2,
    ]);
  }

  notify(method: string, params?: unknown): void {
    void this.requireLanguageClient().sendNotification(method, params);
  }

  didSave(uri: string): void {
    this.notify("textDocument/didSave", {
      textDocument: { uri },
    });
  }

  request(method: string, params?: unknown): Promise<unknown> {
    if (this.disposed) {
      return Promise.reject(new Error(CLIENT_DISPOSED_MESSAGE));
    }
    return this.requireLanguageClient().sendRequest(method, params);
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
        { scheme: "file", language: "systemverilog" },
        { scheme: "file", language: "verilog" },
      ],
      workspaceFolder: {
        index: 0,
        name: "workspace",
        uri: vscode.Uri.parse(this.rootUri),
      },
      initializationOptions: browserInitializationOptions(),
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
    if (!languageClient || !workerReadyStatus || this.disposed || languageClient.isRunning()) {
      return;
    }

    try {
      await languageClient.start();
      this.onServerCapabilities(languageClient.initializeResult?.capabilities ?? null);
      this.onStatus(workerReadyStatus);
    } catch (error) {
      this.onStatus({
        engine: "unavailable",
        ready: false,
        detail: error instanceof Error ? error.message : "Vide language client failed to start.",
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
