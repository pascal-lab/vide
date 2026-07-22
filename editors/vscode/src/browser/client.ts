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

import { videInitializationOptions } from "./shared/initialization-options";
import type {
  LspTraceEntry,
  WorkerRequest,
  WorkerResponse,
  WorkerStatus,
} from "./shared/types";
import {
  BROWSER_WORKSPACE_FOLDER_NAME,
  type BrowserWorkspaceSnapshot,
} from "./workspaceSnapshot";

const CLIENT_DISPOSED_MESSAGE = "Vide browser client has been disposed.";
const RENAME_EXPANSION_INFO_REQUEST =
  "vide.server.renameExpansionInfo";
const EXPANDED_RENAME_REQUEST = "vide.server.expandedRename";
const RENAME_CONFLICT_INFO_REQUEST = "vide.server.renameConflictInfo";

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
        provideRenameEdits: async (document, position, newName, token, next) => {
          const languageClient = this.requireLanguageClient();
          const textDocumentPosition = {
            textDocument:
              languageClient.code2ProtocolConverter.asTextDocumentIdentifier(document),
            position: languageClient.code2ProtocolConverter.asPosition(position),
          };
          const standardRename = async () => {
            if (
              !(await confirmRenameCollision(
                languageClient,
                textDocumentPosition,
                newName,
                false,
                token,
              ))
            ) {
              return emptyRenameEdit();
            }
            return await next(document, position, newName, token);
          };

          let info: RenameExpansionInfo | undefined;
          try {
            info = await languageClient.sendRequest<RenameExpansionInfo>(
              "workspace/executeCommand",
              {
                command: RENAME_EXPANSION_INFO_REQUEST,
                arguments: [{ textDocumentPosition }],
              },
              token,
            );
          } catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.onLog(`Falling back to standard rename: ${message}`, "warn");
          }

          if (!info || info.additionalSymbols === 0) {
            return await standardRename();
          }

          const recursiveAction = vscode.l10n.t(
            "Rename Connected Ports/Signals",
          );
          const localAction = vscode.l10n.t("Only This Symbol");
          const selected = await vscode.window.showInformationMessage(
            vscode.l10n.t(
              "Rename {0} connected port/signal symbol(s) as well?",
              info.additionalSymbols,
            ),
            recursiveAction,
            localAction,
          );

          if (selected === localAction) {
            return await standardRename();
          }

          if (selected !== recursiveAction) {
            return emptyRenameEdit();
          }

          if (
            !(await confirmRenameCollision(
              languageClient,
              textDocumentPosition,
              newName,
              true,
              token,
            ))
          ) {
            return emptyRenameEdit();
          }

          const edit = await languageClient.sendRequest(
              "workspace/executeCommand",
              {
              command: EXPANDED_RENAME_REQUEST,
                arguments: [{ textDocumentPosition, newName }],
              },
            token,
          );
          return await languageClient.protocol2CodeConverter.asWorkspaceEdit(
            edit as never,
            token,
          );
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

type RenameExpansionInfo = {
  additionalSymbols: number;
};

type RenameConflictInfo = {
  conflicts: number;
};

function emptyRenameEdit(): vscode.WorkspaceEdit {
  return new vscode.WorkspaceEdit();
}

async function confirmRenameCollision(
  languageClient: VideLanguageClient,
  textDocumentPosition: unknown,
  newName: string,
  recursive: boolean,
  token: vscode.CancellationToken,
): Promise<boolean> {
  const info = await languageClient.sendRequest<RenameConflictInfo>(
    "workspace/executeCommand",
    {
      command: RENAME_CONFLICT_INFO_REQUEST,
      arguments: [{ textDocumentPosition, newName, recursive }],
    },
    token,
  );

  if (info.conflicts === 0) {
    return true;
  }

  const continueAction = vscode.l10n.t("Continue Rename");
  const cancelAction = vscode.l10n.t("Cancel");
  const selected = await vscode.window.showWarningMessage(
    vscode.l10n.t(
      'Renaming to "{0}" may collide with {1} existing symbol(s).',
      newName,
      info.conflicts,
    ),
    continueAction,
    cancelAction,
  );
  return selected === continueAction;
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
