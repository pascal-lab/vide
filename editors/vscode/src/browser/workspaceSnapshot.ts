import * as vscode from "vscode";

import type { WorkerWorkspaceFile } from "../../../../packages/vide-extension-shared/src/browser/types";
import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_SOURCE_FILE_GLOB,
  isProjectConfigFileName,
  isProjectSourceFileName,
} from "../projectConfigCommon";

const textDecoder = new TextDecoder("utf-8");

export const BROWSER_WORKSPACE_ROOT_URI = "file:///workspace";
export const BROWSER_WORKSPACE_FOLDER_NAME = "workspace";

export interface BrowserWorkspaceSnapshot {
  rootUri: string;
  workspaceRootUris: string[];
  workspaceFiles: WorkerWorkspaceFile[];
}

export async function buildBrowserWorkspaceSnapshot(
  log: (message: string) => void,
): Promise<BrowserWorkspaceSnapshot> {
  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  const filesByUri = new Map<string, WorkerWorkspaceFile>();

  for (const [index, folder] of workspaceFolders.entries()) {
    for (const pattern of [
      PROJECT_SOURCE_FILE_GLOB,
      `**/${PROJECT_CONFIG_FILE_NAME}`,
    ]) {
      const matches = await vscode.workspace.findFiles(
        new vscode.RelativePattern(folder, pattern),
      );
      for (const uri of matches) {
        const syntheticPath = syntheticPathForUri(
          workspaceFolders,
          folder,
          index,
          uri,
        );
        const content = await readWorkspaceText(uri);
        filesByUri.set(uri.toString(), {
          path: syntheticPath,
          text: content,
          uri: uri.toString(),
        });
      }
    }
  }

  for (const document of vscode.workspace.textDocuments) {
    if (!shouldMirrorWorkspaceDocument(document)) {
      continue;
    }
    const folder = vscode.workspace.getWorkspaceFolder(document.uri);
    if (!folder) {
      continue;
    }
    const index = workspaceFolders.findIndex(
      (candidate) => candidate.uri.toString() === folder.uri.toString(),
    );
    if (index < 0) {
      continue;
    }
    filesByUri.set(document.uri.toString(), {
      path: syntheticPathForUri(workspaceFolders, folder, index, document.uri),
      text: document.getText(),
      uri: document.uri.toString(),
    });
  }

  log(
    `[INFO] Prepared browser workspace snapshot with ${filesByUri.size} mirrored files.`,
  );

  return {
    rootUri: BROWSER_WORKSPACE_ROOT_URI,
    workspaceRootUris: workspaceFolders.map((folder) => folder.uri.toString()),
    workspaceFiles: [...filesByUri.values()],
  };
}

export async function createProjectConfigAtRoot(rootUri: string): Promise<vscode.Uri> {
  const root = vscode.Uri.parse(rootUri);
  const configUri = vscode.Uri.joinPath(root, PROJECT_CONFIG_FILE_NAME);

  try {
    await vscode.workspace.fs.stat(configUri);
    return configUri;
  } catch {
    await vscode.workspace.fs.writeFile(
      configUri,
      new TextEncoder().encode(DEFAULT_PROJECT_CONFIG_TEXT),
    );
    return configUri;
  }
}

export function shouldMirrorWorkspaceDocument(
  document: Pick<vscode.TextDocument, "uri" | "fileName">,
): boolean {
  const fileName = baseName(document.fileName || document.uri.path);
  return (
    isProjectSourceFileName(fileName) || isProjectConfigFileName(fileName)
  );
}

export function shouldRestartForWatchedUri(uri: vscode.Uri): boolean {
  const fileName = baseName(uri.path);
  return (
    isProjectSourceFileName(fileName) || isProjectConfigFileName(fileName)
  );
}

function syntheticPathForUri(
  workspaceFolders: readonly vscode.WorkspaceFolder[],
  folder: vscode.WorkspaceFolder,
  index: number,
  uri: vscode.Uri,
): string {
  const relativePath = relativePathWithinFolder(folder, uri);
  return workspaceFolders.length === 1
    ? relativePath
    : `root-${index}/${relativePath}`;
}

function relativePathWithinFolder(
  folder: vscode.WorkspaceFolder,
  uri: vscode.Uri,
): string {
  const folderPath = folder.uri.path.replace(/\/+$/, "");
  const filePath = uri.path;
  const relative =
    filePath === folderPath
      ? ""
      : filePath.startsWith(`${folderPath}/`)
        ? filePath.slice(folderPath.length + 1)
        : filePath.replace(/^\/+/, "");
  return relative
    .split("/")
    .filter(Boolean)
    .map((segment) => decodeURIComponent(segment))
    .join("/");
}

async function readWorkspaceText(uri: vscode.Uri): Promise<string> {
  return textDecoder.decode(await vscode.workspace.fs.readFile(uri));
}

function baseName(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const slashIndex = normalized.lastIndexOf("/");
  return slashIndex >= 0 ? normalized.slice(slashIndex + 1) : normalized;
}
