export interface VizslaScenarioFile {
  path: string;
  source: string;
  languageId?: string;
  editable?: boolean;
}

export interface VizslaScenario {
  id: string;
  label: string;
  entryFile: string;
  description: string;
  files: VizslaScenarioFile[];
}

export type LspSeverity = 1 | 2 | 3 | 4;

export interface LspPosition {
  line: number;
  character: number;
}

export interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

export interface LabDiagnostic {
  uri: string;
  filePath: string;
  range: LspRange;
  severity: LspSeverity;
  tags?: number[];
  source: string;
  title: string;
  code?: string;
  rawCode?: string;
  data?: unknown;
  message: string;
}

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
}

export type WorkerRequest =
  | { kind: "boot"; wasmBaseUrl: string; rootUri: string; workspaceFiles: WorkerWorkspaceFile[]; lspPort: MessagePort }
  | { kind: "stop" };

export type WorkerResponse =
  | { kind: "status"; status: WorkerStatus }
  | { kind: "trace"; entry: LspTraceEntry }
  | { kind: "log"; level: "info" | "warn" | "error"; message: string };
