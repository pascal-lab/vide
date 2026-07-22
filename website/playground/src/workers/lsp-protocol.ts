export interface WasmEngine {
  send(message: LspMessage): LspMessage[];
  poll(): LspMessage[];
  writeFile(path: string, text: string): void;
  reset(): void;
}

export type LspMessage = LspRequest | LspNotification | LspResponse;

export interface LspRequest {
  jsonrpc: "2.0";
  id: number | string;
  method: string;
  params?: unknown;
}

export interface LspNotification {
  jsonrpc: "2.0";
  method: string;
  params?: unknown;
}

export interface LspResponse {
  jsonrpc: "2.0";
  id: number | string;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

export interface PendingClientRequest {
  requestId: number;
  method: string;
  timeout: number;
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
