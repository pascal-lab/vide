import type * as Monaco from "@codingame/monaco-vscode-editor-api";
import type { VideScenario, VideScenarioFile, WorkerWorkspaceFile } from "../types";

export const DEFAULT_WORKSPACE_ROOT_URI = "file:///workspace";

const SOURCE_EXTENSIONS = new Set([".v", ".vh", ".sv", ".svh", ".svi"]);

export interface LabFileState {
  file: VideScenarioFile;
  uri: string;
  model: Monaco.editor.ITextModel;
}

export function scenarioWorkspaceFiles(scenario: VideScenario): WorkerWorkspaceFile[] {
  return scenario.files.map((file) => ({
    path: normalizeWorkspacePath(file.path),
    text: file.source,
  }));
}

export function entryFile(scenario: VideScenario): VideScenarioFile {
  return scenario.files.find((file) => normalizeWorkspacePath(file.path) === normalizeWorkspacePath(scenario.entryFile)) ?? scenario.files[0];
}

export function sourceFiles(scenario: VideScenario): VideScenarioFile[] {
  return scenario.files.filter((file) => isSourceFile(file.path));
}

export function workspaceUri(path: string, rootUri = DEFAULT_WORKSPACE_ROOT_URI): string {
  return `${normalizeRootUri(rootUri)}/${normalizeWorkspacePath(path).split("/").map(encodeURIComponent).join("/")}`;
}

export function pathFromWorkspaceUri(uri: string, rootUri = DEFAULT_WORKSPACE_ROOT_URI): string {
  const prefix = `${normalizeRootUri(rootUri)}/`;
  if (!uri.startsWith(prefix)) {
    return uri;
  }
  return decodeURIComponent(uri.slice(prefix.length));
}

export function displayPath(path: string): string {
  return normalizeWorkspacePath(path);
}

export function fileName(path: string): string {
  const parts = normalizeWorkspacePath(path).split("/");
  return parts[parts.length - 1] ?? path;
}

export function isSourceFile(path: string): boolean {
  return SOURCE_EXTENSIONS.has(extension(path));
}

export function languageIdForPath(path: string): string {
  const ext = extension(path);
  if (ext === ".v" || ext === ".vh") {
    return "verilog";
  }
  if (SOURCE_EXTENSIONS.has(ext)) {
    return "systemverilog";
  }
  return "plaintext";
}

export function normalizeWorkspacePath(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/^\/+/, "");
  const parts = normalized.split("/").filter(Boolean);
  if (parts.some((part) => part === "." || part === "..")) {
    throw new Error(`Invalid workspace path: ${path}`);
  }
  return parts.join("/");
}

export function normalizeRootUri(rootUri: string): string {
  return rootUri.replace(/\/+$/, "");
}

function extension(path: string): string {
  const name = fileName(path).toLowerCase();
  const index = name.lastIndexOf(".");
  return index >= 0 ? name.slice(index) : "";
}
