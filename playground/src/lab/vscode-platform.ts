import { defaultApi } from "vscode/localExtensionHost";
import { initialize, type IWorkbenchConstructionOptions } from "@codingame/monaco-vscode-api";
import { ExtensionHostKind, registerExtension } from "@codingame/monaco-vscode-api/extensions";
import { waitServicesReady } from "@codingame/monaco-vscode-api/lifecycle";
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri";

let startPromise: Promise<void> | undefined;

const defaultApiExtension = registerExtension(
  {
    name: "vizsla-playground-client",
    publisher: "vizsla",
    version: "0.0.0",
    engines: {
      vscode: "*",
    },
  },
  ExtensionHostKind.LocalProcess,
  { system: true },
);

export function startVizslaVscodePlatform(): Promise<void> {
  startPromise ??= initialize({}, undefined, workspaceConfiguration())
    .then(() => waitServicesReady())
    .then(async () => {
      await defaultApiExtension.setAsDefaultApi();
      if (!defaultApi) {
        throw new Error("VS Code extension API default instance was not initialized.");
      }
    });
  return startPromise;
}

function workspaceConfiguration(): IWorkbenchConstructionOptions {
  return {
    workspaceProvider: {
      trusted: true,
      workspace: {
        workspaceUri: URI.parse("file:///workspace.code-workspace"),
      },
      async open() {
        return false;
      },
    },
  };
}
