export interface LspTraceEntry {
  id: number;
  direction: "client" | "server";
  method: string;
  detail: string;
}

export interface WorkerStatus {
  engine: "wasm" | "unavailable";
  ready: boolean;
  detail: string;
}

export interface WorkerWorkspaceFile {
  path: string;
  text: string;
  uri?: string;
}

export type WorkerRequest =
  | {
      kind: "boot";
      wasmBaseUrl: string;
      rootUri: string;
      workspaceRootUris?: string[];
      workspaceFiles: WorkerWorkspaceFile[];
      lspPort: MessagePort;
    }
  | { kind: "stop" };

export type WorkerResponse =
  | { kind: "status"; status: WorkerStatus }
  | { kind: "trace"; entry: LspTraceEntry }
  | { kind: "log"; level: "info" | "warn" | "error"; message: string };
