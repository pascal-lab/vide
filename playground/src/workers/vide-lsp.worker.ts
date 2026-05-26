import type { WorkerRequest, WorkerResponse, WorkerStatus, WorkerWorkspaceFile } from "../types";
import { isRecord, type LspMessage, type LspNotification, type WasmEngine } from "./lsp-protocol";

const REQUEST_TIMEOUT_MS = 15_000;
const POLL_INTERVAL_MS = 16;

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
const pendingLspRequests = new Map<number | string, PendingLspRequest>();
const workspaceTextByPath = new Map<string, string>();

self.addEventListener("message", (event: MessageEvent<WorkerRequest>) => {
  void handleRequest(event.data).catch((error: unknown) => {
    post({
      kind: "log",
      level: "error",
      message: error instanceof Error ? error.message : "Vide worker request failed.",
    });
  });
});

async function handleRequest(message: WorkerRequest): Promise<void> {
  switch (message.kind) {
    case "boot":
      trace("client", "boot", `${message.workspaceFiles.length} workspace files`);
      await boot(message.wasmBaseUrl, message.rootUri, message.workspaceFiles, message.lspPort);
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
  workspaceFiles: WorkerWorkspaceFile[],
  requestedLspPort: MessagePort,
): Promise<void> {
  try {
    stopEngine();
    rootUri = normalizeRootUri(requestedRootUri);
    workspaceTextByPath.clear();
    for (const file of workspaceFiles) {
      workspaceTextByPath.set(normalizeWorkspacePath(file.path), file.text);
    }
    lspPort = requestedLspPort;
    lspPort.onmessage = (event: MessageEvent<LspMessage>) => handleLspMessage(event.data);
    lspPort.start();
    engine = await loadWasmEngine(wasmBaseUrl, rootUri, workspaceFiles);
    status = { engine: "wasm", ready: true, detail: "Vide WASM engine loaded." };
    trace("server", "ready", status.detail);
  } catch (error) {
    stopEngine();
    status = {
      engine: "unavailable",
      ready: false,
      detail: error instanceof Error ? error.message : "Vide WASM is not available.",
    };
    post({
      kind: "log",
      level: "error",
      message: `${status.detail} Run npm run build:wasm in the playground package.`,
    });
  }
}

async function loadWasmEngine(wasmBaseUrl: string, requestedRootUri: string, workspaceFiles: WorkerWorkspaceFile[]): Promise<WasmEngine> {
  const baseUrl = new URL(wasmBaseUrl.endsWith("/") ? wasmBaseUrl : `${wasmBaseUrl}/`, self.location.href);
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

  return loaded.createVideLspEngine({ wasmBaseUrl: baseUrl.href, rootUri: requestedRootUri, workspaceFiles });
}

function handleLspMessage(message: LspMessage): void {
  traceLspMessage("client", message);
  if (!engine) {
    respondWithError(message, status.detail);
    return;
  }

  trackClientRequest(message);
  applyWorkspaceTextSideEffect(message);

  try {
    const emitted = engine.send(message);
    processEmittedMessages(emitted);
    schedulePump();
  } catch (error) {
    clearClientRequest(message);
    respondWithError(message, error instanceof Error ? error.message : "Vide LSP request failed.");
  }
}

function processEmittedMessages(emitted: LspMessage[]): void {
  for (const message of emitted) {
    traceLspMessage("server", message);
    clearClientRequest(message);
    postLsp(message);
  }
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
      failPendingRequests(error instanceof Error ? error.message : "Vide LSP polling failed.");
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
      error: { code: -32001, message: `Vide LSP did not respond to ${pending.method}.` },
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
    const contentChanges = Array.isArray(params?.contentChanges) ? params.contentChanges : [];
    if (!path || contentChanges.length === 0) {
      return;
    }
    let text = workspaceTextByPath.get(path) ?? "";
    for (const change of contentChanges.map(recordValue)) {
      if (!change || typeof change.text !== "string") {
        continue;
      }
      text = change.range ? applyRangedTextChange(text, recordValue(change.range), change.text) : change.text;
    }
    writeWorkspaceFile(path, text);
  }
}

function applyRangedTextChange(text: string, range: Record<string, unknown> | undefined, replacement: string): string {
  const start = recordValue(range?.start);
  const end = recordValue(range?.end);
  const startOffset = offsetAt(text, Number(start?.line), Number(start?.character));
  const endOffset = offsetAt(text, Number(end?.line), Number(end?.character));
  return `${text.slice(0, startOffset)}${replacement}${text.slice(endOffset)}`;
}

function offsetAt(text: string, line: number, character: number): number {
  if (!Number.isFinite(line) || !Number.isFinite(character) || line <= 0) {
    return Math.max(0, Math.min(text.length, Number.isFinite(character) ? character : 0));
  }

  let offset = 0;
  for (let currentLine = 0; currentLine < line && offset < text.length; currentLine += 1) {
    const nextNewline = text.indexOf("\n", offset);
    if (nextNewline < 0) {
      return text.length;
    }
    offset = nextNewline + 1;
  }

  return Math.max(0, Math.min(text.length, offset + character));
}

function lspParams(message: LspNotification): Record<string, unknown> | undefined {
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
}

function traceLspMessage(direction: "client" | "server", message: LspMessage): void {
  if ("method" in message) {
    const detail = "params" in message ? summarizeJson(message.params) : "";
    trace(direction, message.method, detail);
  } else if ("id" in message) {
    trace(direction, `response#${String(message.id)}`, message.error ? message.error.message : "ok");
  }
}

function trace(direction: "client" | "server", method: string, detail: string): void {
  post({ kind: "trace", entry: { id: traceId++, direction, method, detail } });
}

function summarizeJson(value: unknown): string {
  const text = JSON.stringify(value);
  return text.length > 160 ? `${text.slice(0, 157)}...` : text;
}

function post(response: WorkerResponse): void {
  self.postMessage(response);
}

function normalizeRootUri(uri: string): string {
  return uri.replace(/\/+$/, "");
}

function normalizeWorkspacePath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\/+/, "");
}

function recordValue(value: unknown): Record<string, unknown> | undefined {
  return isRecord(value) ? value : undefined;
}
