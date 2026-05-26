import { LitElement, type PropertyValues, type TemplateResult } from "lit";
import type * as Monaco from "@codingame/monaco-vscode-editor-api";
import { FileStripScrollController } from "./file-strip-scroll";
import { renderVizslaLabView, type FileDialogState } from "./vizsla-lab.view";
import { vizslaLabStyles } from "./vizsla-lab.styles";
import { LabDiagnosticController } from "../lab/diagnostics";
import { LabEditorWorkspace } from "../lab/editor-workspace";
import { VizslaBrowserClient } from "../lab/lsp-client";
import { toMarkerData } from "../lab/monaco-lsp";
import { installShadowDomHoverBridge } from "../lab/monaco-shadow-hover";
import {
  configureMonaco,
  setVizslaMonacoTheme,
  syncVizslaSemanticTheme,
  vizslaThemeName,
  wireVizslaVscodeLanguage,
  type VizslaColorScheme,
} from "../lab/monaco-setup";
import { isSourceFile, normalizeWorkspacePath, scenarioWorkspaceFiles, workspaceUri } from "../lab/workspace";
import {
  cloneScenario,
  createFileScenario,
  defaultNewFilePath,
  deleteFileScenario,
  renameFileScenario,
} from "../lab/workspace-mutations";
import { getScenario } from "../scenarios";
import type { LabDiagnostic, VizslaScenario, WorkerStatus } from "../types";

const DIAGNOSTIC_DEBOUNCE_MS = 260;

export class VizslaLabElement extends LitElement {
  static properties = {
    scenario: { type: String },
    wasmBaseUrl: { type: String, attribute: "wasm-base-url" },
    height: { type: String },
    theme: { type: String },
    docs: { type: Boolean, reflect: true },
    project: { attribute: false },
    activeFile: { type: String, attribute: "active-file" },
    cursorLine: { type: Number, attribute: "cursor-line" },
    cursorColumn: { type: Number, attribute: "cursor-column" },
    selection: { type: String },
    diagnosticsOpen: { type: Boolean, attribute: "diagnostics-open", reflect: true },
    focusEditor: { type: Boolean, attribute: "focus-editor" },
  };

  static styles = vizslaLabStyles;

  declare scenario: string;
  declare wasmBaseUrl: string;
  declare height: string;
  declare theme: "auto" | VizslaColorScheme;
  declare docs: boolean;
  declare project: VizslaScenario | undefined;
  declare activeFile: string;
  declare cursorLine: number;
  declare cursorColumn: number;
  declare selection: string;
  declare diagnosticsOpen: boolean;
  declare focusEditor: boolean;

  private monaco?: typeof Monaco;
  private editor?: Monaco.editor.IStandaloneCodeEditor;
  private client?: VizslaBrowserClient;
  private editorDisposables: Monaco.IDisposable[] = [];
  private editorWorkspace?: LabEditorWorkspace;
  private activeScenario: VizslaScenario = getScenario("counter");
  private initialScenario: VizslaScenario = cloneScenario(getScenario("counter"));
  private readonly workspaceRootUri = `file:///workspace-${Math.random().toString(36).slice(2)}`;
  private status: WorkerStatus = { engine: "unavailable", ready: false, detail: "Starting Vizsla WASM engine." };
  private inspectorOpen = false;
  private readonly fileStripScroll = new FileStripScrollController(
    () => this.renderRoot,
    () => this.requestUpdate(),
  );
  private readonly diagnostics = new LabDiagnosticController({
    debounceMs: DIAGNOSTIC_DEBOUNCE_MS,
    client: () => this.client,
    ready: () => this.status.ready,
    state: (uri) => this.editorWorkspace?.state(uri),
    onDidChange: () => this.requestUpdate(),
    onMarkersChanged: (uri) => this.applyMarkers(uri),
  });
  private fileDialog: FileDialogState | undefined;
  private clientGeneration = 0;
  private serverCapabilities: unknown;
  private colorScheme: VizslaColorScheme = "dark";
  private themeObserver?: MutationObserver;
  private themeMediaQuery?: MediaQueryList;
  private readonly handleThemeChange = () => this.syncColorScheme();

  constructor() {
    super();
    this.scenario = "counter";
    this.wasmBaseUrl = "/wasm/";
    this.height = "";
    this.theme = "auto";
    this.docs = false;
    this.project = undefined;
    this.activeFile = "";
    this.cursorLine = 0;
    this.cursorColumn = 1;
    this.selection = "";
    this.diagnosticsOpen = false;
    this.focusEditor = false;
  }

  protected firstUpdated(): void {
    this.activeScenario = cloneScenario(this.resolvedScenario());
    this.initialScenario = cloneScenario(this.activeScenario);
    this.inspectorOpen = this.diagnosticsOpen;
    this.installThemeSync();
    this.syncColorScheme();
    this.syncLabHeight();
    void this.mountEditor()
      .then(() => {
        this.applyConfiguredState();
        this.restartClient();
        this.fileStripScroll.queueMeasurement();
      })
      .catch((error: unknown) => {
        this.status = {
          engine: "unavailable",
          ready: false,
          detail: error instanceof Error ? error.message : "Failed to start the editor runtime.",
        };
        this.requestUpdate();
      });
  }

  protected updated(changed: PropertyValues<this>): void {
    if (changed.has("height") || changed.has("docs")) {
      this.syncLabHeight();
    }

    if (changed.has("theme")) {
      this.syncColorScheme();
    }

    if ((changed.has("scenario") || changed.has("project")) && this.editor) {
      this.setScenario(this.resolvedScenario(), true, true);
    }

    if (
      this.editor &&
      (changed.has("activeFile") ||
        changed.has("cursorLine") ||
        changed.has("cursorColumn") ||
        changed.has("selection") ||
        changed.has("focusEditor"))
    ) {
      this.applyConfiguredState();
    }

    if (changed.has("diagnosticsOpen") && this.inspectorOpen !== this.diagnosticsOpen) {
      this.inspectorOpen = this.diagnosticsOpen;
      this.requestUpdate();
    }

    this.fileStripScroll.queueMeasurement();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback();
    this.fileStripScroll.dispose();
    this.disposeEditorDisposables();
    this.editor?.dispose();
    this.editorWorkspace?.dispose();
    this.diagnostics.dispose();
    this.client?.dispose();
    this.themeObserver?.disconnect();
    this.removeThemeMediaListener();
  }

  protected render(): TemplateResult {
    return renderVizslaLabView(
      {
        activeScenario: this.activeScenario,
        activeUri: this.editorWorkspace?.activeUri ?? "",
        workspaceRootUri: this.workspaceRootUri,
        diagnosticsByUri: this.diagnostics.diagnosticsByUri,
        status: this.status,
        inspectorOpen: this.inspectorOpen,
        diagnosticsBusy: this.diagnostics.busy,
        ...this.fileStripScroll.state,
        fileDialog: this.fileDialog,
      },
      {
        updateFileStripScroll: (event) => this.fileStripScroll.updateScroll(event),
        jumpFileStripScrollbar: (event) => this.fileStripScroll.jumpScrollbar(event),
        beginFileStripThumbDrag: (event) => this.fileStripScroll.beginThumbDrag(event),
        createFile: () => this.createFile(),
        renameFile: () => this.renameFile(),
        deleteFile: () => this.deleteFile(),
        updateFileDialogValue: (event) => this.updateFileDialogValue(event),
        submitFileDialog: (event) => this.submitFileDialog(event),
        closeFileDialog: () => this.closeFileDialog(),
        refreshDiagnostics: () => void this.refreshDiagnosticsNow(),
        resetScenario: () => this.resetScenario(),
        copySource: () => void this.copySource(),
        activateFile: (uri) => this.activateFile(uri),
        revealDiagnostic: (diagnostic) => this.revealDiagnostic(diagnostic),
        toggleDiagnostics: () => this.toggleDiagnostics(),
        closeInspector: () => this.closeInspector(),
      },
    );
  }

  private async mountEditor(): Promise<void> {
    this.monaco = await configureMonaco();
    const editorHost = this.renderRoot.querySelector<HTMLElement>(".editor");
    if (!editorHost) {
      return;
    }

    this.createModels(this.activeScenario);
    this.editor = this.monaco.editor.create(editorHost, {
      model: this.activeFileState()?.model,
      theme: vizslaThemeName(this.colorScheme),
      automaticLayout: true,
      fontFamily: "'Cascadia Code', 'SFMono-Regular', Consolas, monospace",
      fontSize: 14,
      lineHeight: 22,
      minimap: { enabled: false },
      renderLineHighlight: "all",
      scrollBeyondLastLine: false,
      tabSize: 2,
      padding: { top: 14, bottom: 14 },
      fixedOverflowWidgets: true,
      hover: { enabled: "on", delay: 250, sticky: true },
      "semanticHighlighting.enabled": true,
    });

    this.editorDisposables.push(
      this.monaco.editor.registerEditorOpener({
        openCodeEditor: (source, resource, selectionOrPosition) => {
          if (source !== this.editor) {
            return false;
          }

          const target = this.editorWorkspace?.state(resource.toString());
          if (!target || !this.editor) {
            return false;
          }

          this.activateFile(target.uri);
          this.revealLocation(selectionOrPosition);
          this.editor.focus();
          return true;
        },
      }),
    );

    this.editorDisposables.push(
      this.editor.onDidChangeModelContent(() => {
        const state = this.activeFileState();
        if (!state) {
          return;
        }
        this.diagnostics.queueSave(state.uri);
        this.scheduleDiagnostics(this.sourceUris());
      }),
      installShadowDomHoverBridge({
        monaco: this.monaco,
        editor: this.editor,
        root: this.renderRoot as ShadowRoot,
        ownsModel: (model) => this.ownsSourceModel(model),
      }),
    );

    void wireVizslaVscodeLanguage(this.editor).catch((error: unknown) => {
      console.warn(error instanceof Error ? error.message : "Failed to load VS Code grammar assets.");
    });
  }

  private createModels(scenario: VizslaScenario): void {
    if (!this.monaco) {
      return;
    }
    if (!this.editorWorkspace) {
      this.editorWorkspace = new LabEditorWorkspace(this.monaco, this.workspaceRootUri, scenario);
      return;
    }
    this.editorWorkspace.replaceScenario(scenario);
  }

  private syncLanguageServerCapabilities(serverCapabilities: unknown): void {
    if (!this.monaco || !this.status.ready) {
      return;
    }

    syncVizslaSemanticTheme(this.monaco, serverCapabilities, this.colorScheme);
  }

  private restartClient(): void {
    this.diagnostics.clearPending();
    this.client?.dispose();
    const generation = ++this.clientGeneration;
    const client = new VizslaBrowserClient(this.wasmBaseUrl, this.workspaceRootUri);
    this.client = client;
    this.serverCapabilities = undefined;
    this.status = { engine: "unavailable", ready: false, detail: "Starting Vizsla WASM engine." };
    client.onStatus = (status) => {
      if (generation !== this.clientGeneration || client !== this.client) {
        return;
      }
      this.status = status;
      if (status.ready) {
        if (this.serverCapabilities) {
          this.syncLanguageServerCapabilities(this.serverCapabilities);
        }
        this.scheduleDiagnostics(this.sourceUris());
      }
      this.requestUpdate();
    };
    client.onServerCapabilities = (capabilities) => {
      if (generation !== this.clientGeneration || client !== this.client) {
        return;
      }
      this.serverCapabilities = capabilities;
      if (this.status.ready) {
        this.syncLanguageServerCapabilities(capabilities);
      }
    };
    client.onLog = (message, level) => {
      if (generation !== this.clientGeneration || client !== this.client) {
        return;
      }
      const logger = level === "error" ? console.error : level === "warn" ? console.warn : console.info;
      logger(message);
    };
    client.start(scenarioWorkspaceFiles(this.activeScenario));
  }

  private async refreshDiagnosticsNow(): Promise<void> {
    await this.diagnostics.refresh(this.sourceUris());
  }

  private scheduleDiagnostics(uris: string[]): void {
    this.diagnostics.schedule(uris);
  }

  private resetScenario(): void {
    this.setScenario(this.initialScenario, true);
  }

  private setScenario(scenario: VizslaScenario, force = false, updateInitial = false, activePath?: string): void {
    if (!force && scenario.id === this.activeScenario.id) {
      return;
    }
    const nextScenario = cloneScenario(scenario);
    this.client?.dispose();
    this.client = undefined;
    this.clientGeneration += 1;
    this.serverCapabilities = undefined;
    this.activeScenario = nextScenario;
    if (updateInitial) {
      this.initialScenario = cloneScenario(nextScenario);
    }
    if (this.scenario !== nextScenario.id) {
      this.scenario = nextScenario.id;
    }
    this.diagnostics.reset();
    this.createModels(nextScenario);
    if (activePath) {
      this.editorWorkspace?.setActivePath(activePath);
    }
    this.editor?.setModel(this.activeFileState()?.model ?? null);
    this.editor?.updateOptions({ readOnly: this.activeFileState()?.file.editable === false });
    if (activePath) {
      this.editor?.focus();
    } else {
      this.applyConfiguredState();
    }
    this.restartClient();
    this.requestUpdate();
  }

  private disposeEditorDisposables(): void {
    this.editorDisposables.forEach((disposable) => disposable.dispose());
    this.editorDisposables = [];
  }

  private activateFile(uri: string): void {
    const state = this.editorWorkspace?.setActiveUri(uri);
    if (!state || !this.editor) {
      return;
    }
    this.editor.setModel(state.model);
    this.editor.updateOptions({ readOnly: state.file.editable === false });
    if (isSourceFile(state.file.path)) {
      this.scheduleDiagnostics(this.sourceUris());
    }
    this.requestUpdate();
  }

  private applyConfiguredState(): void {
    this.applyConfiguredFile();
    this.applyConfiguredSelection();
    if (this.focusEditor) {
      this.editor?.focus();
    }
  }

  private applyConfiguredFile(): void {
    const uri = this.configuredActiveUri();
    if (!uri || uri === this.editorWorkspace?.activeUri) {
      return;
    }
    this.activateFile(uri);
  }

  private configuredActiveUri(): string | undefined {
    if (!this.activeFile) {
      return undefined;
    }

    let uri: string;
    try {
      uri = this.workspaceUri(normalizeWorkspacePath(this.activeFile));
    } catch (error) {
      console.warn(error instanceof Error ? error.message : "Invalid active-file value.");
      return undefined;
    }

    if (!this.editorWorkspace?.state(uri)) {
      console.warn(`active-file '${this.activeFile}' is not part of scenario '${this.activeScenario.id}'.`);
      return undefined;
    }

    return uri;
  }

  private applyConfiguredSelection(): void {
    if (!this.editor) {
      return;
    }

    const range = this.configuredRange();
    if (range) {
      this.editor.setSelection(range);
      this.editor.revealRangeInCenterIfOutsideViewport(range);
      return;
    }

    if (this.cursorLine >= 1) {
      const model = this.editor.getModel();
      if (!model) {
        return;
      }
      const position = model.validatePosition({
        lineNumber: this.cursorLine,
        column: Math.max(1, this.cursorColumn || 1),
      });
      this.editor.setPosition(position);
      this.editor.revealLineInCenterIfOutsideViewport(position.lineNumber);
    }
  }

  private revealLocation(selectionOrPosition: Monaco.IRange | Monaco.IPosition | undefined): void {
    if (!this.editor || !this.monaco || !selectionOrPosition) {
      return;
    }

    if ("startLineNumber" in selectionOrPosition) {
      const range = new this.monaco.Range(
        selectionOrPosition.startLineNumber,
        selectionOrPosition.startColumn,
        selectionOrPosition.endLineNumber,
        selectionOrPosition.endColumn,
      );
      this.editor.setSelection(range);
      this.editor.revealRangeInCenter(range);
      return;
    }

    const position = {
      lineNumber: selectionOrPosition.lineNumber,
      column: selectionOrPosition.column,
    };
    this.editor.setPosition(position);
    this.editor.revealPositionInCenter(position);
  }

  private configuredRange(): Monaco.Range | undefined {
    if (!this.selection || !this.monaco || !this.editor) {
      return undefined;
    }

    const match = /^(\d+):(\d+)-(\d+):(\d+)$/.exec(this.selection.trim());
    if (!match) {
      console.warn(`Invalid selection '${this.selection}'. Expected line:column-line:column.`);
      return undefined;
    }

    const model = this.editor.getModel();
    if (!model) {
      return undefined;
    }

    const start = model.validatePosition({
      lineNumber: Number(match[1]),
      column: Number(match[2]),
    });
    const end = model.validatePosition({
      lineNumber: Number(match[3]),
      column: Number(match[4]),
    });

    return new this.monaco.Range(start.lineNumber, start.column, end.lineNumber, end.column);
  }

  private revealDiagnostic(diagnostic: LabDiagnostic): void {
    const state = this.editorWorkspace?.state(diagnostic.uri);
    if (!state || !this.editor) {
      return;
    }
    this.activateFile(diagnostic.uri);
    const range = new this.monaco!.Range(
      diagnostic.range.start.line + 1,
      diagnostic.range.start.character + 1,
      diagnostic.range.end.line + 1,
      diagnostic.range.end.character + 1,
    );
    this.editor.setSelection(range);
    this.editor.revealRangeInCenter(range);
  }

  private async copySource(): Promise<void> {
    await navigator.clipboard.writeText(this.activeFileState()?.model.getValue() ?? "");
  }

  private createFile(): void {
    this.fileDialog = { mode: "create", value: this.defaultNewFilePath() };
    this.requestUpdate();
  }

  private renameFile(): void {
    const state = this.activeFileState();
    if (!state) {
      return;
    }

    this.fileDialog = { mode: "rename", value: state.file.path, targetPath: state.file.path };
    this.requestUpdate();
  }

  private deleteFile(): void {
    const state = this.activeFileState();
    if (!state) {
      return;
    }
    if (this.activeScenario.files.length <= 1) {
      this.fileDialog = {
        mode: "delete",
        value: state.file.path,
        targetPath: state.file.path,
        error: "The workspace must keep at least one file.",
      };
      this.requestUpdate();
      return;
    }

    this.fileDialog = { mode: "delete", value: state.file.path, targetPath: state.file.path };
    this.requestUpdate();
  }

  private updateFileDialogValue(event: Event): void {
    if (!this.fileDialog || !(event.currentTarget instanceof HTMLInputElement)) {
      return;
    }
    this.fileDialog = {
      ...this.fileDialog,
      value: event.currentTarget.value,
      error: undefined,
    };
    this.requestUpdate();
  }

  private submitFileDialog(event: Event): void {
    event.preventDefault();
    const dialog = this.fileDialog;
    if (!dialog) {
      return;
    }

    if (dialog.mode === "delete") {
      this.commitDeleteFile(dialog.targetPath);
      return;
    }

    const path = this.validatedDialogPath(dialog);
    if (!path) {
      return;
    }

    if (dialog.mode === "create") {
      this.commitCreateFile(path);
    } else {
      this.commitRenameFile(dialog.targetPath, path);
    }
  }

  private closeFileDialog(): void {
    this.fileDialog = undefined;
    this.requestUpdate();
    this.editor?.focus();
  }

  private commitCreateFile(path: string): void {
    const mutation = createFileScenario(this.activeScenario, this.currentWorkspaceFiles(), path);
    this.fileDialog = undefined;
    this.setScenario(mutation.scenario, true, false, mutation.activePath);
  }

  private commitRenameFile(currentPath: string | undefined, nextPath: string): void {
    const state = this.activeFileState();
    const fromPath = currentPath ?? state?.file.path;
    if (!fromPath) {
      this.setFileDialogError("No active file to rename.");
      return;
    }
    if (nextPath === fromPath) {
      this.fileDialog = undefined;
      this.requestUpdate();
      return;
    }

    const mutation = renameFileScenario(this.activeScenario, this.currentWorkspaceFiles(), fromPath, nextPath);
    this.fileDialog = undefined;
    this.setScenario(mutation.scenario, true, false, mutation.activePath);
  }

  private commitDeleteFile(path: string | undefined): void {
    if (!path) {
      this.setFileDialogError("No active file to delete.");
      return;
    }
    const mutation = deleteFileScenario(this.activeScenario, this.currentWorkspaceFiles(), path);
    if ("error" in mutation) {
      this.setFileDialogError(mutation.error);
      return;
    }
    this.fileDialog = undefined;
    this.setScenario(mutation.scenario, true, false, mutation.activePath);
  }

  private toggleDiagnostics(): void {
    this.inspectorOpen = !this.inspectorOpen;
    this.diagnosticsOpen = this.inspectorOpen;
    this.requestUpdate();
  }

  private closeInspector(): void {
    this.inspectorOpen = false;
    this.diagnosticsOpen = false;
    this.requestUpdate();
  }

  private applyMarkers(uri: string): void {
    if (!this.monaco) {
      return;
    }
    const state = this.editorWorkspace?.state(uri);
    if (!state) {
      return;
    }
    const diagnostics = this.diagnostics.diagnosticsByUri.get(uri) ?? [];
    this.monaco.editor.setModelMarkers(
      state.model,
      "vizsla",
      diagnostics.map((diagnostic) => toMarkerData(this.monaco!, diagnostic)),
    );
  }

  private activeFileState() {
    return this.editorWorkspace?.activeState();
  }

  private ownsSourceModel(model: Monaco.editor.ITextModel): boolean {
    return this.editorWorkspace?.ownsSourceModel(model) ?? false;
  }

  private sourceUris(): string[] {
    return this.editorWorkspace?.sourceUris() ?? [];
  }

  private workspaceUri(path: string): string {
    return this.editorWorkspace?.uriForPath(path) ?? workspaceUri(path, this.workspaceRootUri);
  }

  private resolvedScenario(): VizslaScenario {
    return this.project ?? getScenario(this.scenario);
  }

  private currentWorkspaceFiles(): VizslaScenario["files"] {
    return this.editorWorkspace?.currentFiles(this.activeScenario) ?? this.activeScenario.files;
  }

  private hasWorkspacePath(path: string): boolean {
    if (this.editorWorkspace) {
      return this.editorWorkspace.hasPath(path);
    }
    const normalized = normalizeWorkspacePath(path);
    return this.activeScenario.files.some((file) => normalizeWorkspacePath(file.path) === normalized);
  }

  private validatedDialogPath(dialog: FileDialogState): string | undefined {
    let path: string;
    try {
      path = normalizeWorkspacePath(dialog.value.trim());
    } catch (error) {
      this.setFileDialogError(error instanceof Error ? error.message : "Invalid workspace path.");
      return undefined;
    }

    if (!path) {
      this.setFileDialogError("Enter a file path inside the virtual workspace.");
      return undefined;
    }
    if (this.hasWorkspacePath(path) && path !== dialog.targetPath) {
      this.setFileDialogError(`A file already exists at '${path}'.`);
      return undefined;
    }

    return path;
  }

  private setFileDialogError(error: string): void {
    if (!this.fileDialog) {
      return;
    }
    this.fileDialog = { ...this.fileDialog, error };
    this.requestUpdate();
  }

  private defaultNewFilePath(): string {
    return defaultNewFilePath((path) => this.hasWorkspacePath(path));
  }

  private installThemeSync(): void {
    if (typeof document !== "undefined") {
      this.themeObserver = new MutationObserver(this.handleThemeChange);
      this.themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["class", "data-theme"],
      });
    }

    if (typeof window !== "undefined" && "matchMedia" in window) {
      this.themeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      this.themeMediaQuery.addEventListener("change", this.handleThemeChange);
    }
  }

  private removeThemeMediaListener(): void {
    if (!this.themeMediaQuery) {
      return;
    }

    this.themeMediaQuery.removeEventListener("change", this.handleThemeChange);
  }

  private syncColorScheme(): void {
    const colorScheme = this.resolveColorScheme();
    if (this.colorScheme === colorScheme && this.getAttribute("data-theme") === colorScheme) {
      return;
    }

    this.colorScheme = colorScheme;
    this.setAttribute("data-theme", colorScheme);
    if (this.monaco) {
      setVizslaMonacoTheme(this.monaco, colorScheme);
    }
    this.requestUpdate();
  }

  private resolveColorScheme(): VizslaColorScheme {
    if (this.theme === "light" || this.theme === "dark") {
      return this.theme;
    }

    if (typeof document !== "undefined") {
      const root = document.documentElement;
      const declaredTheme = (root.dataset.theme ?? root.getAttribute("data-theme") ?? "").toLowerCase();
      if (declaredTheme === "light" || declaredTheme === "dark") {
        return declaredTheme;
      }
      if (root.classList.contains("dark")) {
        return "dark";
      }
      if (root.classList.contains("light")) {
        return "light";
      }
    }

    return this.themeMediaQuery?.matches ? "dark" : "light";
  }

  private syncLabHeight(): void {
    this.style.setProperty("--vzlab-height", this.height || (this.docs ? "430px" : "100dvh"));
  }

}

if (!customElements.get("vizsla-lab")) {
  customElements.define("vizsla-lab", VizslaLabElement);
}
