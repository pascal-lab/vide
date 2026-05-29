import type { WorkerRequest, WorkerResponse, WorkerStatus, WorkerWorkspaceFile } from "./types";
import { isRecord, type LspMessage, type LspNotification, type WasmEngine } from "./lsp-protocol";

const REQUEST_TIMEOUT_MS = 15_000;
const POLL_INTERVAL_MS = 16;
const RUN_QIHE_ANALYSIS_COMMAND = "vide.server.runQiheAnalysis";

interface PendingLspRequest {
  method: string;
  timeout: number;
}

let engine: WasmEngine | null = null;
let lspPort: MessagePort | undefined;
let status: WorkerStatus = {
  engine: "unavailable",
  ready: false,
  detail: "Vide WASM engine has not been loaded.",
};
let traceId = 1;
let pollTimer: number | undefined;
let rootUri = "file:///workspace";
let externalWorkspaceRootUris: string[] = [];
const pendingLspRequests = new Map<number | string, PendingLspRequest>();
const workspaceTextByPath = new Map<string, string>();
const externalToSyntheticUris = new Map<string, string>();
const syntheticToExternalUris = new Map<string, string>();

export function installVideWorkerRuntime(): void {
  self.addEventListener("message", (event: MessageEvent<WorkerRequest>) => {
    void handleRequest(event.data).catch((error: unknown) => {
      post({
        kind: "log",
        level: "error",
        message:
          error instanceof Error ? error.message : "Vide worker request failed.",
      });
    });
  });
}

async function handleRequest(message: WorkerRequest): Promise<void> {
  switch (message.kind) {
    case "boot":
      trace("client", "boot", `${message.workspaceFiles.length} workspace files`);
      await boot(
        message.wasmBaseUrl,
        message.rootUri,
        message.workspaceRootUris ?? [],
        message.workspaceFiles,
        message.lspPort,
      );
      post({ kind: "status", status });
      break;
    case "stop":
      stopEngine();
      break;
  }
}

async function boot(
  wasmBaseUrl: string,
  requestedRootUri: string,
  requestedWorkspaceRootUris: string[],
  workspaceFiles: WorkerWorkspaceFile[],
  requestedLspPort: MessagePort,
): Promise<void> {
  try {
    stopEngine();
    rootUri = normalizeRootUri(requestedRootUri);
    externalWorkspaceRootUris = requestedWorkspaceRootUris
      .map(normalizeRootUri)
      .sort((left, right) => right.length - left.length);
    workspaceTextByPath.clear();
    for (const [index, workspaceRootUri] of requestedWorkspaceRootUris.entries()) {
      registerUriMapping(
        normalizeRootUri(workspaceRootUri),
        syntheticRootUriForIndex(requestedWorkspaceRootUris.length, index),
      );
    }
    for (const file of workspaceFiles) {
      const normalizedPath = normalizeWorkspacePath(file.path);
      const syntheticUri = workspaceUri(normalizedPath);
      const externalUri = normalizeRootUri(file.uri ?? syntheticUri);
      registerUriMapping(externalUri, syntheticUri);
      workspaceTextByPath.set(normalizedPath, file.text);
    }
    lspPort = requestedLspPort;
    lspPort.onmessage = (event: MessageEvent<LspMessage>) => handleLspMessage(event.data);
    lspPort.start();
    engine = await loadWasmEngine(wasmBaseUrl, rootUri, workspaceFiles);
    status = {
      engine: "wasm",
      ready: true,
      detail: "Vide WASM engine loaded.",
    };
    trace("server", "ready", status.detail);
  } catch (error) {
    stopEngine();
    status = {
      engine: "unavailable",
      ready: false,
      detail:
        error instanceof Error ? error.message : "Vide WASM is not available.",
    };
    post({
      kind: "log",
      level: "error",
      message: `${status.detail} Run npm run build:wasm before using the browser runtime.`,
    });
  }
}

async function loadWasmEngine(
  wasmBaseUrl: string,
  requestedRootUri: string,
  workspaceFiles: WorkerWorkspaceFile[],
): Promise<WasmEngine> {
  const baseUrl = new URL(
    wasmBaseUrl.endsWith("/") ? wasmBaseUrl : `${wasmBaseUrl}/`,
    self.location.href,
  );
  const moduleUrl = new URL("vide-lsp.js", baseUrl);
  moduleUrl.search = baseUrl.search;
  const loaded = (await import(/* @vite-ignore */ moduleUrl.href)) as {
    createVideLspEngine?: (options: {
      wasmBaseUrl: string;
      rootUri: string;
      workspaceFiles: WorkerWorkspaceFile[];
    }) => Promise<WasmEngine>;
  };

  if (!loaded.createVideLspEngine) {
    throw new Error("Vide WASM adapter did not export createVideLspEngine().");
  }

  return loaded.createVideLspEngine({
    wasmBaseUrl: baseUrl.href,
    rootUri: requestedRootUri,
    workspaceFiles,
  });
}

function handleLspMessage(message: LspMessage): void {
  const translatedMessage = rewriteMessageUris(
    message,
    translateClientUriToSynthetic,
  );
  traceLspMessage("client", translatedMessage);
  if (!engine) {
    respondWithError(translatedMessage, status.detail);
    return;
  }

  trackClientRequest(translatedMessage);
  applyWorkspaceTextSideEffect(translatedMessage);

  try {
    const emitted = engine.send(translatedMessage);
    processEmittedMessages(emitted);
    schedulePump();
  } catch (error) {
    clearClientRequest(translatedMessage);
    respondWithError(
      translatedMessage,
      error instanceof Error ? error.message : "Vide LSP request failed.",
    );
  }
}

function processEmittedMessages(emitted: LspMessage[]): void {
  for (const rawMessage of emitted) {
    const message = disableUnsupportedBrowserCapabilities(rawMessage);
    const translatedMessage = rewriteMessageUris(
      message,
      translateSyntheticUriToClient,
    );
    traceLspMessage("server", translatedMessage);
    clearClientRequest(translatedMessage);
    postLsp(translatedMessage);
  }
}

function disableUnsupportedBrowserCapabilities(message: LspMessage): LspMessage {
  if (!isInitializeResponse(message)) {
    return message;
  }

  const capabilities = message.result.capabilities;
  delete capabilities.documentFormattingProvider;
  delete capabilities.documentRangeFormattingProvider;
  delete capabilities.documentOnTypeFormattingProvider;

  const executeCommandProvider = recordValue(capabilities.executeCommandProvider);
  if (executeCommandProvider && Array.isArray(executeCommandProvider.commands)) {
    executeCommandProvider.commands = executeCommandProvider.commands.filter(
      (command) => command !== RUN_QIHE_ANALYSIS_COMMAND,
    );
  }

  return message;
}

function isInitializeResponse(
  message: LspMessage,
): message is LspMessage & { result: { capabilities: Record<string, unknown> } } {
  if (!("id" in message) || "method" in message || !isRecord(message.result)) {
    return false;
  }
  const pending = pendingLspRequests.get(message.id);
  return pending?.method === "initialize" && isRecord(message.result.capabilities);
}

function pollLsp(): void {
  if (!engine) {
    return;
  }
  processEmittedMessages(engine.poll());
}

function schedulePump(): void {
  if (pollTimer !== undefined || pendingLspRequests.size === 0) {
    return;
  }

  pollTimer = self.setTimeout(() => {
    pollTimer = undefined;
    try {
      pollLsp();
      schedulePump();
    } catch (error) {
      failPendingRequests(
        error instanceof Error ? error.message : "Vide LSP polling failed.",
      );
    }
  }, POLL_INTERVAL_MS);
}

function trackClientRequest(message: LspMessage): void {
  if (!("id" in message) || !("method" in message)) {
    return;
  }

  const id = message.id;
  const method = message.method;
  const timeout = self.setTimeout(() => {
    const pending = pendingLspRequests.get(id);
    if (!pending) {
      return;
    }
    pendingLspRequests.delete(id);
    trace("server", pending.method, "request timed out");
    postLsp({
      jsonrpc: "2.0",
      id,
      error: {
        code: -32001,
        message: `Vide LSP did not respond to ${pending.method}.`,
      },
    });
  }, REQUEST_TIMEOUT_MS);

  pendingLspRequests.set(id, { method, timeout });
}

function clearClientRequest(message: LspMessage): void {
  if (!("id" in message) || "method" in message) {
    return;
  }
  const pending = pendingLspRequests.get(message.id);
  if (!pending) {
    return;
  }
  self.clearTimeout(pending.timeout);
  pendingLspRequests.delete(message.id);
}

function failPendingRequests(message: string): void {
  for (const [id, pending] of pendingLspRequests) {
    self.clearTimeout(pending.timeout);
    postLsp({
      jsonrpc: "2.0",
      id,
      error: { code: -32001, message },
    });
  }
  pendingLspRequests.clear();
}

function respondWithError(message: LspMessage, errorMessage: string): void {
  if (!("id" in message)) {
    post({ kind: "log", level: "error", message: errorMessage });
    return;
  }
  postLsp({
    jsonrpc: "2.0",
    id: message.id,
    error: { code: -32001, message: errorMessage },
  });
}

function postLsp(message: LspMessage): void {
  lspPort?.postMessage(message);
}

function writeWorkspaceFile(path: string, text: string): void {
  const normalized = normalizeWorkspacePath(path);
  workspaceTextByPath.set(normalized, text);
  requireEngine().writeFile(normalized, text);
}

function applyWorkspaceTextSideEffect(message: LspMessage): void {
  if (!("method" in message)) {
    return;
  }

  if (message.method === "textDocument/didOpen") {
    const params = lspParams(message);
    const textDocument = recordValue(params?.textDocument);
    const path = workspacePathFromUri(textDocument?.uri);
    if (path && typeof textDocument?.text === "string") {
      writeWorkspaceFile(path, textDocument.text);
    }
    return;
  }

  if (message.method === "textDocument/didChange") {
    const params = lspParams(message);
    const textDocument = recordValue(params?.textDocument);
    const path = workspacePathFromUri(textDocument?.uri);
    const contentChanges = Array.isArray(params?.contentChanges)
      ? params.contentChanges
      : [];
    if (!path || contentChanges.length === 0) {
      return;
    }
    let text = workspaceTextByPath.get(path) ?? "";
    for (const change of contentChanges.map(recordValue)) {
      if (!change || typeof change.text !== "string") {
        continue;
      }
      text = change.range
        ? applyRangedTextChange(text, recordValue(change.range), change.text)
        : change.text;
    }
    writeWorkspaceFile(path, text);
  }
}

function applyRangedTextChange(
  text: string,
  range: Record<string, unknown> | undefined,
  replacement: string,
): string {
  const start = recordValue(range?.start);
  const end = recordValue(range?.end);
  const startOffset = offsetAt(text, Number(start?.line), Number(start?.character));
  const endOffset = offsetAt(text, Number(end?.line), Number(end?.character));
  return `${text.slice(0, startOffset)}${replacement}${text.slice(endOffset)}`;
}

function offsetAt(text: string, line: number, character: number): number {
  if (!Number.isFinite(line) || !Number.isFinite(character) || line <= 0) {
    return Math.max(
      0,
      Math.min(text.length, Number.isFinite(character) ? character : 0),
    );
  }

  let offset = 0;
  for (
    let currentLine = 0;
    currentLine < line && offset < text.length;
    currentLine += 1
  ) {
    const nextNewline = text.indexOf("\n", offset);
    if (nextNewline < 0) {
      return text.length;
    }
    offset = nextNewline + 1;
  }

  return Math.max(0, Math.min(text.length, offset + character));
}

function lspParams(
  message: LspNotification,
): Record<string, unknown> | undefined {
  return recordValue(message.params);
}

function workspacePathFromUri(uri: unknown): string | undefined {
  if (typeof uri !== "string") {
    return undefined;
  }
  const prefix = `${rootUri}/`;
  if (!uri.startsWith(prefix)) {
    return undefined;
  }
  return normalizeWorkspacePath(decodeURIComponent(uri.slice(prefix.length)));
}

function requireEngine(): WasmEngine {
  if (!engine) {
    throw new Error(status.detail);
  }
  return engine;
}

function stopEngine(): void {
  failPendingRequests("Vide LSP is stopping.");
  if (pollTimer !== undefined) {
    self.clearTimeout(pollTimer);
    pollTimer = undefined;
  }
  engine?.reset();
  engine = null;
  lspPort?.close();
  lspPort = undefined;
  externalWorkspaceRootUris = [];
  externalToSyntheticUris.clear();
  syntheticToExternalUris.clear();
  workspaceTextByPath.clear();
}

function traceLspMessage(direction: "client" | "server", message: LspMessage): void {
  if ("method" in message) {
    const detail = "params" in message ? summarizeJson(message.params) : "";
    trace(direction, message.method, detail);
  } else if ("id" in message) {
    trace(
      direction,
      `response#${String(message.id)}`,
      message.error ? message.error.message : "ok",
    );
  }
}

function trace(direction: "client" | "server", method: string, detail: string): void {
  post({
    kind: "trace",
    entry: { id: traceId++, direction, method, detail },
  });
}

function summarizeJson(value: unknown): string {
  const text = JSON.stringify(value);
  return text.length > 160 ? `${text.slice(0, 157)}...` : text;
}

function post(response: WorkerResponse): void {
  self.postMessage(response);
}

function registerUriMapping(externalUri: string, syntheticUri: string): void {
  externalToSyntheticUris.set(externalUri, syntheticUri);
  syntheticToExternalUris.set(syntheticUri, externalUri);
}

function translateClientUriToSynthetic(uri: string): string {
  const direct = externalToSyntheticUris.get(uri);
  if (direct) {
    return direct;
  }

  for (const workspaceRoot of externalWorkspaceRootUris) {
    if (uri === workspaceRoot || uri.startsWith(`${workspaceRoot}/`)) {
      const relative = decodeURIComponent(
        uri === workspaceRoot ? "" : uri.slice(workspaceRoot.length + 1),
      );
      const syntheticUri = workspaceUri(relative);
      registerUriMapping(uri, syntheticUri);
      return syntheticUri;
    }
  }

  return uri;
}

function translateSyntheticUriToClient(uri: string): string {
  return syntheticToExternalUris.get(uri) ?? uri;
}

function rewriteMessageUris<T>(
  value: T,
  translate: (uri: string) => string,
): T {
  if (Array.isArray(value)) {
    return value.map((item) => rewriteMessageUris(item, translate)) as T;
  }

  if (!isRecord(value)) {
    return value;
  }

  const output: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    if (typeof item === "string" && (key === "uri" || key.endsWith("Uri"))) {
      output[key] = translate(item);
    } else if (
      Array.isArray(item) &&
      key.endsWith("Uris") &&
      item.every((entry) => typeof entry === "string")
    ) {
      output[key] = item.map((entry) => translate(entry));
    } else {
      output[key] = rewriteMessageUris(item, translate);
    }
  }
  return output as T;
}

function normalizeRootUri(uri: string): string {
  return uri.replace(/\/+$/, "");
}

function syntheticRootUriForIndex(rootCount: number, index: number): string {
  return rootCount === 1 ? rootUri : workspaceUri(`root-${index}`);
}

function normalizeWorkspacePath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\/+/, "");
}

function workspaceUri(path: string): string {
  return `${rootUri}/${normalizeWorkspacePath(path)
    .split("/")
    .map(encodeURIComponent)
    .join("/")}`;
}

function recordValue(value: unknown): Record<string, unknown> | undefined {
  return isRecord(value) ? value : undefined;
}
