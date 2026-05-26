import { css, unsafeCSS, type CSSResultGroup } from "lit";
import monacoStyles from "monaco-editor/min/vs/editor/editor.main.css?inline";

export const vizslaLabStyles: CSSResultGroup = [
  unsafeCSS(monacoStyles),
  css`
    :host {
      display: block;
      color: #09090b;
      color-scheme: light;
      font-family:
        "Aptos",
        "Segoe UI",
        system-ui,
        sans-serif;
      --vzlab-height: 100dvh;
      --vzlab-background: #fafafa;
      --vzlab-panel: #ffffff;
      --vzlab-editor: #ffffff;
      --vzlab-border: #e4e4e7;
      --vzlab-border-strong: #d4d4d8;
      --vzlab-muted: #71717a;
      --vzlab-message: #3f3f46;
      --vzlab-muted-surface: #f4f4f5;
      --vzlab-accent: #18181b;
      --vzlab-ring: #a1a1aa;
      --vzlab-danger: #dc2626;
      --vzlab-warning: #b45309;
      --vzlab-success: #16a34a;
    }

    :host([data-theme="dark"]) {
      color: #fafafa;
      color-scheme: dark;
      --vzlab-background: #09090b;
      --vzlab-panel: #09090b;
      --vzlab-editor: #0a0a0a;
      --vzlab-border: #27272a;
      --vzlab-border-strong: #3f3f46;
      --vzlab-muted: #a1a1aa;
      --vzlab-message: #d4d4d8;
      --vzlab-muted-surface: #18181b;
      --vzlab-accent: #fafafa;
      --vzlab-ring: #71717a;
      --vzlab-danger: #f87171;
      --vzlab-warning: #fbbf24;
      --vzlab-success: #22c55e;
    }

    :host([docs]) {
      --vzlab-height: 430px;
    }

    :host,
    .shell,
    .body,
    .editor-panel,
    .workspace-row,
    .file-strip-shell,
    .file-strip,
    .file-strip button,
    .file-strip-scrollbar,
    .file-strip-thumb,
    .toolbar,
    .toolbar button,
    .drawer,
    .drawer-header,
    .drawer-header button,
    .panel,
    .empty,
    .diagnostic,
    .status,
    .badge,
    .status-dot,
    .dialog-backdrop,
    .file-dialog,
    .file-dialog *,
    .file-dialog *::before,
    .file-dialog *::after {
      box-sizing: border-box;
    }

    .shell {
      position: relative;
      height: var(--vzlab-height);
      min-height: 0;
      display: grid;
      grid-template-rows: 1fr;
      overflow: hidden;
      background: var(--vzlab-background);
      border: 0;
      border-radius: 0;
    }

    :host([docs]) .shell {
      border: 1px solid var(--vzlab-border);
      border-radius: 8px;
    }

    .body,
    .editor-panel,
    .editor {
      min-width: 0;
      min-height: 0;
    }

    .body {
      display: grid;
      background: var(--vzlab-editor);
    }

    .editor-panel {
      display: grid;
      grid-template-rows: auto minmax(0, 1fr) auto;
    }

    .monaco-editor .reference-zone-widget .messages,
    .monaco-editor .reference-zone-widget .ref-tree,
    .monaco-editor .reference-zone-widget .ref-tree .monaco-list,
    .monaco-editor .reference-zone-widget .reference,
    .monaco-editor .reference-zone-widget .referenceMatch,
    .monaco-editor .reference-zone-widget .monaco-icon-label,
    .monaco-editor .reference-zone-widget .monaco-icon-label .label-name,
    .monaco-editor .reference-zone-widget .monaco-icon-label .label-description,
    .monaco-editor .reference-zone-widget .count {
      font-size: 12px;
    }

    .monaco-editor .action-widget {
      width: min(520px, calc(100vw - 32px)) !important;
      min-width: min(360px, calc(100vw - 32px));
      padding: 4px;
      border-color: var(--vzlab-border-strong) !important;
      border-radius: 8px;
      box-shadow:
        0 12px 28px rgba(24, 24, 27, 0.14),
        0 2px 8px rgba(24, 24, 27, 0.08);
    }

    .monaco-editor .action-widget .monaco-list-row.action {
      min-height: 28px;
      padding: 0 8px;
    }

    .monaco-editor .action-widget .monaco-list-row.action .title {
      font-size: 12px;
      line-height: 1.35;
    }

    .monaco-editor .action-widget .monaco-list .monaco-list-row .description {
      font-size: 11px;
    }

    .workspace-row {
      min-height: 36px;
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      align-items: stretch;
      background: var(--vzlab-panel);
      border-bottom: 1px solid var(--vzlab-border);
    }

    .file-strip-shell {
      position: relative;
      min-width: 0;
      min-height: 36px;
      overflow: hidden;
      background: var(--vzlab-panel);
    }

    .file-strip {
      width: 100%;
      min-width: 0;
      display: flex;
      align-items: stretch;
      overflow-x: auto;
      overflow-y: hidden;
      scrollbar-width: none;
      -ms-overflow-style: none;
    }

    .file-strip::-webkit-scrollbar {
      width: 0;
      height: 0;
      display: none;
    }

    .file-strip-scrollbar {
      position: absolute;
      inset-inline: 8px;
      bottom: 2px;
      height: 4px;
      border-radius: 999px;
      background: color-mix(in srgb, var(--vzlab-border-strong), transparent 58%);
      opacity: 0;
      pointer-events: none;
      transition:
        opacity 160ms ease,
        background 160ms ease;
    }

    .file-strip-thumb {
      position: absolute;
      inset-block: 0;
      border-radius: inherit;
      background: color-mix(in srgb, var(--vzlab-accent), transparent 44%);
      transition: background 160ms ease;
      cursor: grab;
    }

    .file-strip-shell.is-overflowing:hover .file-strip-scrollbar,
    .file-strip-shell.is-overflowing:focus-within .file-strip-scrollbar,
    .file-strip-shell.is-scrolling .file-strip-scrollbar,
    .file-strip-shell.is-dragging .file-strip-scrollbar {
      opacity: 1;
      pointer-events: auto;
    }

    .file-strip-scrollbar:hover,
    .file-strip-shell.is-dragging .file-strip-scrollbar {
      background: color-mix(in srgb, var(--vzlab-border-strong), transparent 36%);
    }

    .file-strip-thumb:hover,
    .file-strip-shell.is-dragging .file-strip-thumb {
      background: color-mix(in srgb, var(--vzlab-accent), transparent 24%);
    }

    .file-strip-shell.is-dragging .file-strip-thumb {
      cursor: grabbing;
    }

    .file-strip button {
      flex: 0 0 auto;
      max-width: 210px;
      height: 36px;
      min-width: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 0;
      border-right: 1px solid var(--vzlab-border);
      border-radius: 0;
      background: transparent;
      color: var(--vzlab-muted);
      padding: 0 10px;
      font:
        500 11px/1 "Cascadia Code",
        Consolas,
        monospace;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .file-strip button:hover,
    .file-strip button:focus-visible {
      background: var(--vzlab-muted-surface);
      color: var(--vzlab-accent);
    }

    .file-strip button.is-active {
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
      box-shadow: inset 0 -2px 0 var(--vzlab-accent);
    }

    .file-strip button.has-diagnostic {
      color: var(--vzlab-warning);
    }

    .file-strip button.has-error {
      color: var(--vzlab-danger);
    }

    .toolbar {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 4px;
      border-left: 1px solid var(--vzlab-border);
      background: var(--vzlab-panel);
    }

    .toolbar button,
    .drawer-header button,
    .status,
    .diagnostic span {
      font:
        500 11px/1 "Cascadia Code",
        Consolas,
        monospace;
    }

    .toolbar button,
    .drawer-header button {
      height: 28px;
      min-width: 28px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 4px;
      border: 1px solid var(--vzlab-border);
      border-radius: 6px;
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
      cursor: pointer;
      transition:
        background 140ms ease,
        border-color 140ms ease,
        color 140ms ease,
        transform 140ms ease;
    }

    .toolbar button:hover,
    .toolbar button:focus-visible,
    .drawer-header button:hover,
    .drawer-header button:focus-visible {
      background: var(--vzlab-muted-surface);
      border-color: var(--vzlab-ring);
      outline: none;
    }

    .toolbar button:active,
    .drawer-header button:active {
      transform: translateY(1px);
    }

    .toolbar button svg,
    .drawer-header button svg {
      width: 14px;
      height: 14px;
    }

    .toolbar button.is-busy svg {
      animation: spin 700ms linear infinite;
    }

    .diagnostics-toggle {
      min-width: 42px;
      padding: 0 7px;
    }

    .diagnostics-toggle.is-active {
      background: var(--vzlab-accent);
      border-color: var(--vzlab-accent);
      color: var(--vzlab-panel);
    }

    .badge {
      min-width: 18px;
      height: 16px;
      display: inline-grid;
      place-items: center;
      border-radius: 999px;
      background: var(--vzlab-muted-surface);
      color: var(--vzlab-muted);
      padding: 0 4px;
      font-size: 10px;
      font-variant-numeric: tabular-nums;
    }

    .diagnostics-toggle.is-active .badge {
      background: rgba(255, 255, 255, 0.16);
      color: #ffffff;
    }

    .status {
      width: 28px;
      height: 28px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      padding: 0;
      border: 1px solid var(--vzlab-border);
      border-radius: 999px;
      background: var(--vzlab-panel);
      color: var(--vzlab-warning);
      flex: 0 0 auto;
    }

    .status.is-ready {
      color: var(--vzlab-success);
    }

    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: currentColor;
    }

    .drawer {
      min-height: 0;
      max-height: min(220px, 36dvh);
      display: grid;
      grid-template-rows: auto 1fr;
      overflow: hidden;
      background: var(--vzlab-panel);
      border-top: 1px solid var(--vzlab-border);
    }

    .drawer-header {
      min-height: 34px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 3px 4px 3px 10px;
      border-bottom: 1px solid var(--vzlab-border);
      background: var(--vzlab-panel);
    }

    .drawer-header > div {
      min-width: 0;
      display: flex;
      align-items: baseline;
      gap: 8px;
    }

    .drawer-header strong {
      color: var(--vzlab-accent);
      font-size: 11px;
      line-height: 1.2;
    }

    .drawer-header span {
      color: var(--vzlab-muted);
      font-size: 11px;
      white-space: nowrap;
    }

    .panel {
      min-height: 0;
      display: none;
      overflow: auto;
      padding: 0;
    }

    .panel.is-active {
      display: block;
    }

    .empty {
      min-height: 90px;
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      color: var(--vzlab-muted);
      text-align: center;
      font-size: 12px;
    }

    .empty svg {
      width: 14px;
      height: 14px;
    }

    .diagnostic {
      display: grid;
      grid-template-columns: minmax(96px, 0.35fr) minmax(0, 1fr) auto;
      align-items: center;
      gap: 12px;
      width: 100%;
      height: auto;
      min-height: 36px;
      border: 0;
      border-left: 2px solid var(--vzlab-danger);
      border-bottom: 1px solid var(--vzlab-border);
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
      border-radius: 0;
      padding: 5px 10px 5px 8px;
      margin: 0;
      text-align: left;
      cursor: pointer;
    }

    .diagnostic:hover,
    .diagnostic:focus-visible {
      background: var(--vzlab-muted-surface);
      outline: none;
    }

    .diagnostic.severity-2 {
      border-left-color: var(--vzlab-warning);
    }

    .diagnostic strong {
      display: block;
      font-size: 12px;
      line-height: 1.3;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .diagnostic p {
      min-width: 0;
      margin: 0;
      color: var(--vzlab-message);
      font-size: 12px;
      line-height: 1.35;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .diagnostic span {
      color: var(--vzlab-muted);
      font-size: 10px;
      white-space: nowrap;
      overflow-wrap: anywhere;
    }

    .dialog-backdrop {
      position: absolute;
      inset: 0;
      z-index: 20;
      display: grid;
      place-items: center;
      padding: 20px;
      background: rgba(9, 9, 11, 0.38);
      backdrop-filter: blur(5px);
    }

    .file-dialog {
      width: min(460px, calc(100vw - 40px));
      display: grid;
      gap: 14px;
      border: 1px solid var(--vzlab-border-strong);
      border-radius: 8px;
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
      padding: 14px;
      box-shadow:
        0 24px 64px rgba(24, 24, 27, 0.22),
        0 4px 16px rgba(24, 24, 27, 0.14);
    }

    .file-dialog-header {
      display: flex;
      align-items: start;
      justify-content: space-between;
      gap: 12px;
    }

    .file-dialog-header > div {
      min-width: 0;
      display: grid;
      gap: 4px;
    }

    .file-dialog-header strong {
      font-size: 13px;
      line-height: 1.3;
    }

    .file-dialog-header span,
    .file-dialog-field span,
    .file-dialog-error,
    .file-dialog-target {
      color: var(--vzlab-muted);
      font-size: 12px;
      line-height: 1.45;
    }

    .file-dialog-header .icon-button {
      flex: 0 0 auto;
      height: 28px;
      min-width: 28px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border: 1px solid var(--vzlab-border);
      border-radius: 6px;
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
      cursor: pointer;
    }

    .file-dialog-header .icon-button:hover,
    .file-dialog-header .icon-button:focus-visible {
      background: var(--vzlab-muted-surface);
      border-color: var(--vzlab-ring);
      outline: none;
    }

    .file-dialog-header .icon-button svg {
      width: 14px;
      height: 14px;
    }

    .file-dialog-field {
      display: grid;
      gap: 6px;
    }

    .file-dialog-field input,
    .file-dialog-target {
      width: 100%;
      border: 1px solid var(--vzlab-border);
      border-radius: 6px;
      background: var(--vzlab-editor);
      color: var(--vzlab-accent);
      font:
        500 12px/1.4 "Cascadia Code",
        Consolas,
        monospace;
      padding: 9px 10px;
    }

    .file-dialog-field input:focus {
      border-color: var(--vzlab-ring);
      outline: 2px solid color-mix(in srgb, var(--vzlab-ring), transparent 62%);
      outline-offset: 1px;
    }

    .file-dialog-target {
      margin: 0;
      overflow-wrap: anywhere;
    }

    .file-dialog-error {
      margin: -2px 0 0;
      color: var(--vzlab-danger);
    }

    .file-dialog-actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
    }

    .file-dialog-actions button {
      min-height: 30px;
      border: 1px solid var(--vzlab-border);
      border-radius: 6px;
      cursor: pointer;
      font:
        600 12px/1 "Aptos",
        "Segoe UI",
        system-ui,
        sans-serif;
      padding: 0 11px;
    }

    .file-dialog-actions .secondary {
      background: var(--vzlab-panel);
      color: var(--vzlab-accent);
    }

    .file-dialog-actions .primary,
    .file-dialog-actions .danger {
      background: var(--vzlab-accent);
      border-color: var(--vzlab-accent);
      color: var(--vzlab-panel);
    }

    .file-dialog-actions .danger {
      background: var(--vzlab-danger);
      border-color: var(--vzlab-danger);
      color: #ffffff;
    }

    .file-dialog-actions button:hover,
    .file-dialog-actions button:focus-visible {
      border-color: var(--vzlab-ring);
      outline: none;
    }

    .file-dialog-actions button:disabled {
      cursor: not-allowed;
      opacity: 0.54;
    }

    @media (max-width: 920px) {
      .workspace-row {
        grid-template-columns: 1fr;
      }

      .toolbar {
        border-top: 1px solid var(--vzlab-border);
        border-left: 0;
        justify-content: flex-end;
        flex-wrap: wrap;
      }

      .toolbar select {
        min-width: 150px;
      }

      .diagnostic {
        grid-template-columns: 1fr;
        align-items: start;
        gap: 3px;
        padding: 7px 10px 7px 8px;
      }

      .diagnostic p,
      .diagnostic strong {
        white-space: normal;
      }

      .diagnostic span {
        white-space: normal;
      }

      .dialog-backdrop {
        padding: 12px;
      }

      .file-dialog {
        width: min(100%, 460px);
      }
    }

    @keyframes spin {
      to {
        transform: rotate(360deg);
      }
    }

    @media (prefers-reduced-motion: reduce) {
      .toolbar button,
      .drawer-header button,
      .toolbar button::before,
      .toolbar button::after,
      .drawer-header button::before,
      .drawer-header button::after,
      .file-strip-scrollbar,
      .file-strip-thumb {
        animation-duration: 1ms !important;
        transition-duration: 1ms !important;
      }
    }
  `,
];
