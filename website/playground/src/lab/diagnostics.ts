import type { LabDiagnostic } from "../types";
import { diagnosticsFromLspReport } from "./monaco-lsp";
import type { VideBrowserClient } from "./lsp-client";
import { displayPath, isSourceFile, type LabFileState } from "./workspace";

export interface LabDiagnosticControllerOptions {
  debounceMs: number;
  client(): VideBrowserClient | undefined;
  ready(): boolean;
  state(uri: string): LabFileState | undefined;
  onDidChange(): void;
  onMarkersChanged(uri: string): void;
}

export class LabDiagnosticController {
  readonly diagnosticsByUri = new Map<string, LabDiagnostic[]>();
  busy = false;

  private pendingSaveUris = new Set<string>();
  private timer: number | undefined;
  private generation = 0;

  constructor(private readonly options: LabDiagnosticControllerOptions) {}

  queueSave(uri: string): void {
    this.pendingSaveUris.add(uri);
  }

  schedule(uris: string[]): void {
    this.clearTimer();
    this.timer = window.setTimeout(() => {
      const saveUris = [...this.pendingSaveUris];
      this.pendingSaveUris.clear();
      void this.refresh(uris, saveUris);
    }, this.options.debounceMs);
  }

  async refresh(uris: string[], saveUris: string[] = []): Promise<void> {
    const client = this.options.client();
    if (!client || !this.options.ready() || (uris.length === 0 && saveUris.length === 0)) {
      return;
    }

    const generation = ++this.generation;
    this.busy = true;
    this.options.onDidChange();

    try {
      for (const uri of saveUris) {
        if (this.options.state(uri)) {
          client.didSave(uri);
        }
      }

      for (const uri of uris) {
        const state = this.options.state(uri);
        if (!state || !isSourceFile(state.file.path)) {
          continue;
        }
        const report = await client.request("textDocument/diagnostic", {
          textDocument: { uri },
          previousResultId: null,
        });
        if (generation !== this.generation) {
          return;
        }
        this.diagnosticsByUri.set(uri, diagnosticsFromLspReport(report, uri, displayPath(state.file.path)));
        this.options.onMarkersChanged(uri);
      }
    } catch (error) {
      console.error(error instanceof Error ? error.message : "Diagnostics failed.");
    } finally {
      if (generation === this.generation) {
        this.busy = false;
        this.options.onDidChange();
      }
    }
  }

  clearPending(): void {
    this.clearTimer();
    this.pendingSaveUris.clear();
  }

  reset(): void {
    this.clearPending();
    this.generation += 1;
    this.busy = false;
    this.diagnosticsByUri.clear();
    this.options.onDidChange();
  }

  dispose(): void {
    this.clearPending();
  }

  private clearTimer(): void {
    if (this.timer !== undefined) {
      window.clearTimeout(this.timer);
      this.timer = undefined;
    }
  }
}
