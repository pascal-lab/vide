import type * as Monaco from "@codingame/monaco-vscode-editor-api";
import type { VideScenario } from "../types";
import {
  entryFile,
  isSourceFile,
  languageIdForPath,
  normalizeWorkspacePath,
  workspaceUri,
  type LabFileState,
} from "./workspace";

export class LabEditorWorkspace {
  readonly fileStates = new Map<string, LabFileState>();
  activeUri: string;

  constructor(
    private readonly monaco: typeof Monaco,
    private readonly rootUri: string,
    scenario: VideScenario,
  ) {
    this.activeUri = this.uriForPath(entryFile(scenario).path);
    this.replaceScenario(scenario);
  }

  replaceScenario(scenario: VideScenario): void {
    this.dispose();
    for (const file of scenario.files) {
      const uri = this.uriForPath(file.path);
      const model = this.monaco.editor.createModel(
        file.source,
        file.languageId ?? languageIdForPath(file.path),
        this.monaco.Uri.parse(uri),
      );
      this.fileStates.set(uri, { file, uri, model });
    }
    this.activeUri = this.uriForPath(entryFile(scenario).path);
  }

  dispose(): void {
    for (const state of this.fileStates.values()) {
      state.model.dispose();
    }
    this.fileStates.clear();
  }

  uriForPath(path: string): string {
    return workspaceUri(path, this.rootUri);
  }

  state(uri: string): LabFileState | undefined {
    return this.fileStates.get(uri);
  }

  activeState(): LabFileState | undefined {
    return this.state(this.activeUri);
  }

  setActiveUri(uri: string): LabFileState | undefined {
    const state = this.state(uri);
    if (!state) {
      return undefined;
    }
    this.activeUri = uri;
    return state;
  }

  setActivePath(path: string): boolean {
    return !!this.setActiveUri(this.uriForPath(path));
  }

  ownsSourceModel(model: Monaco.editor.ITextModel): boolean {
    const state = this.state(model.uri.toString());
    return !!state && isSourceFile(state.file.path);
  }

  sourceUris(): string[] {
    return [...this.fileStates.values()].filter((state) => isSourceFile(state.file.path)).map((state) => state.uri);
  }

  currentFiles(scenario: VideScenario): VideScenario["files"] {
    return scenario.files.map((file) => {
      const state = this.state(this.uriForPath(file.path));
      return {
        ...file,
        source: state?.model.getValue() ?? file.source,
      };
    });
  }

  hasPath(path: string): boolean {
    const normalized = normalizeWorkspacePath(path);
    return [...this.fileStates.values()].some((state) => normalizeWorkspacePath(state.file.path) === normalized);
  }
}
