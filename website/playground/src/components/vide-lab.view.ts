import { html, type TemplateResult } from "lit";
import { ClipboardCopy, FilePlus, Pencil, RefreshCw, RotateCcw, SearchCode, Trash2, X } from "lucide";
import { displayPath, workspaceUri } from "../lab/workspace";
import type { LabDiagnostic, VideScenario, VideScenarioFile, WorkerStatus } from "../types";
import { renderIcon as icon } from "./icons";

export interface FileDialogState {
  mode: "create" | "rename" | "delete";
  value: string;
  targetPath?: string;
  error?: string;
}

interface VideLabViewState {
  activeScenario: VideScenario;
  activeUri: string;
  workspaceRootUri: string;
  diagnosticsByUri: ReadonlyMap<string, LabDiagnostic[]>;
  status: WorkerStatus;
  inspectorOpen: boolean;
  diagnosticsBusy: boolean;
  fileStripOverflowing: boolean;
  fileStripScrolling: boolean;
  fileStripDragging: boolean;
  fileStripThumbLeft: number;
  fileStripThumbWidth: number;
  fileDialog?: FileDialogState;
}

interface VideLabViewActions {
  updateFileStripScroll(event: Event): void;
  jumpFileStripScrollbar(event: PointerEvent): void;
  beginFileStripThumbDrag(event: PointerEvent): void;
  createFile(): void;
  renameFile(): void;
  deleteFile(): void;
  updateFileDialogValue(event: Event): void;
  submitFileDialog(event: Event): void;
  closeFileDialog(): void;
  refreshDiagnostics(): void;
  resetScenario(): void;
  copySource(): void;
  activateFile(uri: string): void;
  revealDiagnostic(diagnostic: LabDiagnostic): void;
  toggleDiagnostics(): void;
  closeInspector(): void;
}

export function renderVideLabView(state: VideLabViewState, actions: VideLabViewActions): TemplateResult {
  const diagnostics = allDiagnostics(state);
  const statusLabel = state.status.ready ? "Ready" : "Starting";
  const fileStripShellClass = [
    "file-strip-shell",
    state.fileStripOverflowing ? "is-overflowing" : "",
    state.fileStripScrolling ? "is-scrolling" : "",
    state.fileStripDragging ? "is-dragging" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return html`
    <section class="shell" aria-label="Vide Lab">
      <div class="body">
        <section class="editor-panel" aria-label="SystemVerilog editor">
          <div class="workspace-row">
            <div class=${fileStripShellClass}>
              <div class="file-strip" role="tablist" aria-label="Workspace files" @scroll=${actions.updateFileStripScroll}>
                ${state.activeScenario.files.map((file) => renderFileTab(file, state, actions))}
              </div>
              <div
                class="file-strip-scrollbar"
                aria-hidden="true"
                @pointerdown=${actions.jumpFileStripScrollbar}
              >
                <span
                  class="file-strip-thumb"
                  style=${`inline-size: ${state.fileStripThumbWidth}%; inset-inline-start: ${state.fileStripThumbLeft}%;`}
                  @pointerdown=${actions.beginFileStripThumbDrag}
                ></span>
              </div>
            </div>
            <div class="toolbar">
              <button type="button" @click=${actions.createFile} title="New virtual file">${icon(FilePlus)}</button>
              <button type="button" @click=${actions.renameFile} title="Rename current file">${icon(Pencil)}</button>
              <button type="button" @click=${actions.deleteFile} title="Delete current file">${icon(Trash2)}</button>
              <button
                class=${state.inspectorOpen ? "diagnostics-toggle is-active" : "diagnostics-toggle"}
                type="button"
                @click=${actions.toggleDiagnostics}
                title="Show diagnostics"
              >
                ${icon(SearchCode)}
                <span class="badge">${diagnostics.length}</span>
              </button>
              <button
                class=${state.diagnosticsBusy ? "is-busy" : ""}
                type="button"
                @click=${actions.refreshDiagnostics}
                title="Refresh diagnostics"
              >
                ${icon(RefreshCw)}
              </button>
              <div
                class=${state.status.ready ? "status is-ready" : "status"}
                title=${state.status.detail}
                role="status"
                aria-label=${`Vide ${statusLabel}: ${state.status.detail}`}
              >
                <span class="status-dot"></span>
              </div>
              <button type="button" @click=${actions.resetScenario} title="Reset workspace">${icon(RotateCcw)}</button>
              <button type="button" @click=${actions.copySource} title="Copy current file">${icon(ClipboardCopy)}</button>
            </div>
          </div>
          <div class="editor"></div>
          ${state.inspectorOpen
            ? html`
                <aside class="drawer" aria-label="Diagnostics">
                  <div class="drawer-header">
                    <div>
                      <strong>Diagnostics</strong>
                      <span>${diagnostics.length === 1 ? "1 diagnostic" : `${diagnostics.length} diagnostics`}</span>
                    </div>
                    <button type="button" @click=${actions.closeInspector} title="Close inspector">${icon(X)}</button>
                  </div>
                  <div class="panel is-active">${renderDiagnostics(diagnostics, actions)}</div>
                </aside>
              `
            : null}
        </section>
      </div>
      ${state.fileDialog ? renderFileDialog(state.fileDialog, actions) : null}
    </section>
  `;
}

function renderFileDialog(dialog: FileDialogState, actions: VideLabViewActions): TemplateResult {
  const isDelete = dialog.mode === "delete";
  const title =
    dialog.mode === "create" ? "New virtual file" : dialog.mode === "rename" ? "Rename virtual file" : "Delete virtual file";
  const description =
    dialog.mode === "create"
      ? "Create a file in this browser workspace. Use paths like rtl/new_module.sv."
      : dialog.mode === "rename"
        ? "Move the current file to a new path inside this browser workspace."
        : `Remove ${dialog.targetPath ?? "the current file"} from this browser workspace.`;
  const confirmLabel = dialog.mode === "create" ? "Create" : dialog.mode === "rename" ? "Rename" : "Delete";
  const submitDisabled = isDelete && !!dialog.error;

  return html`
    <div class="dialog-backdrop" role="presentation" @click=${actions.closeFileDialog}>
      <form
        class="file-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="file-dialog-title"
        @submit=${actions.submitFileDialog}
        @click=${(event: Event) => event.stopPropagation()}
      >
        <div class="file-dialog-header">
          <div>
            <strong id="file-dialog-title">${title}</strong>
            <span>${description}</span>
          </div>
          <button type="button" class="icon-button" @click=${actions.closeFileDialog} title="Close">${icon(X)}</button>
        </div>

        ${isDelete
          ? html`<p class="file-dialog-target">${dialog.targetPath}</p>`
          : html`
              <label class="file-dialog-field">
                <span>File path</span>
                <input
                  name="path"
                  autocomplete="off"
                  spellcheck="false"
                  autofocus
                  .value=${dialog.value}
                  @input=${actions.updateFileDialogValue}
                />
              </label>
            `}

        ${dialog.error ? html`<p class="file-dialog-error">${dialog.error}</p>` : null}

        <div class="file-dialog-actions">
          <button type="button" class="secondary" @click=${actions.closeFileDialog}>Cancel</button>
          <button type="submit" class=${isDelete ? "danger" : "primary"} ?disabled=${submitDisabled}>${confirmLabel}</button>
        </div>
      </form>
    </div>
  `;
}

function renderFileTab(
  file: VideScenarioFile,
  state: VideLabViewState,
  actions: VideLabViewActions,
): TemplateResult {
  const uri = workspaceUri(file.path, state.workspaceRootUri);
  const diagnostics = state.diagnosticsByUri.get(uri) ?? [];
  const classes = [
    uri === state.activeUri ? "is-active" : "",
    diagnostics.length > 0 ? "has-diagnostic" : "",
    diagnostics.some((diagnostic) => diagnostic.severity === 1) ? "has-error" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return html`
    <button type="button" role="tab" class=${classes} @click=${() => actions.activateFile(uri)} title=${displayPath(file.path)}>
      ${displayPath(file.path)}
    </button>
  `;
}

function renderDiagnostics(
  diagnostics: LabDiagnostic[],
  actions: VideLabViewActions,
): TemplateResult | TemplateResult[] {
  if (diagnostics.length === 0) {
    return html`<div class="empty">${icon(SearchCode)}<span>No diagnostics</span></div>`;
  }

  return diagnostics.map(
    (diagnostic) => html`
      <button type="button" class="diagnostic severity-${diagnostic.severity}" @click=${() => actions.revealDiagnostic(diagnostic)}>
        <strong>${diagnostic.title}</strong>
        <p>${diagnostic.message}</p>
        <span>${diagnostic.source} - ${diagnostic.filePath}:${diagnostic.range.start.line + 1}:${diagnostic.range.start.character + 1}</span>
      </button>
    `,
  );
}

function allDiagnostics(state: VideLabViewState): LabDiagnostic[] {
  return Array.from(state.diagnosticsByUri.values()).flat();
}
