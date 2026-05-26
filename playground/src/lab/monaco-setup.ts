import * as monaco from "@codingame/monaco-vscode-editor-api";
import EditorWorker from "@codingame/monaco-vscode-editor-api/esm/vs/editor/editor.worker?worker&inline";
import { createOnigScanner, createOnigString, loadWASM } from "vscode-oniguruma";
import onigWasm from "vscode-oniguruma/release/onig.wasm?url";
import { INITIAL, parseRawGrammar, Registry, type IOnigLib, type StateStack } from "vscode-textmate";
import languageConfiguration from "../../../editors/vscode/language-configuration.json";
import systemVerilogGrammar from "../../../editors/vscode/syntaxes/systemverilog.tmLanguage.json?raw";
import verilogGrammar from "../../../editors/vscode/syntaxes/verilog.tmLanguage.json?raw";
import { startVizslaVscodePlatform } from "./vscode-platform";

let configured = false;
let cancellationBoundaryInstalled = false;
let onigLibPromise: Promise<IOnigLib> | null = null;
let semanticTokenTypes: string[] = [];
let activeColorScheme: VizslaColorScheme = "dark";

export type VizslaColorScheme = "light" | "dark";

export async function configureMonaco(): Promise<typeof monaco> {
  await startVizslaVscodePlatform();

  if (!configured) {
    installExpectedCancellationBoundary();
    installShadowCaretRangeFromPoint();

    self.MonacoEnvironment = {
      getWorker() {
        return new EditorWorker();
      },
    };

    monaco.languages.register({
      id: "systemverilog",
      extensions: [".sv", ".svh", ".svi"],
      aliases: ["SystemVerilog", "systemverilog"],
    });
    monaco.languages.register({
      id: "verilog",
      extensions: [".v", ".vh"],
      aliases: ["Verilog", "verilog"],
    });

    defineVizslaThemes(monaco, []);

    configured = true;
  }

  return monaco;
}

function installExpectedCancellationBoundary(): void {
  if (cancellationBoundaryInstalled || typeof window === "undefined") {
    return;
  }
  cancellationBoundaryInstalled = true;

  const preventIfExpectedCancellation = (event: ErrorEvent | PromiseRejectionEvent) => {
    const error = "reason" in event ? event.reason : event.error;
    if (isExpectedMonacoCancellation(error)) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  };

  window.addEventListener("error", preventIfExpectedCancellation, true);
  window.addEventListener("unhandledrejection", preventIfExpectedCancellation, true);
}

function isExpectedMonacoCancellation(error: unknown): boolean {
  return error instanceof Error && error.name === "Canceled" && error.message === "Canceled";
}

export function syncVizslaSemanticTheme(
  monacoModule: typeof monaco,
  serverCapabilities: unknown,
  colorScheme: VizslaColorScheme = activeColorScheme,
): void {
  semanticTokenTypes = semanticTokenTypesFromCapabilities(serverCapabilities);
  defineVizslaThemes(monacoModule, semanticTokenTypes);
  setVizslaMonacoTheme(monacoModule, colorScheme);
}

export function setVizslaMonacoTheme(monacoModule: typeof monaco, colorScheme: VizslaColorScheme): void {
  activeColorScheme = colorScheme;
  monacoModule.editor.setTheme(vizslaThemeName(colorScheme));
}

export function vizslaThemeName(colorScheme: VizslaColorScheme): string {
  return colorScheme === "dark" ? "vizsla-lab-dark" : "vizsla-lab-light";
}

function defineVizslaThemes(monacoModule: typeof monaco, semanticTokenTypes: readonly string[]): void {
  const semanticModifierRules = Array.from(new Set(semanticTokenTypes)).flatMap((tokenType) => [
    { token: `${tokenType}.read`, fontStyle: "bold" },
    { token: `${tokenType}.write`, fontStyle: "bold underline" },
  ]);

  monacoModule.editor.defineTheme("vizsla-lab-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "keyword", foreground: "93c5fd" },
      { token: "keyword.preprocessor", foreground: "fbbf24" },
      { token: "type.identifier", foreground: "5eead4" },
      { token: "number", foreground: "fca5a5" },
      ...semanticModifierRules,
    ],
    colors: {
      "editor.background": "#0a0a0a",
      "editor.foreground": "#e4e4e7",
      "editorLineNumber.foreground": "#71717a",
      "editorCursor.foreground": "#fafafa",
      "editor.selectionBackground": "#3f3f46",
      "editor.inactiveSelectionBackground": "#27272a",
      "editorGutter.background": "#0a0a0a",
      "editorLineNumber.activeForeground": "#fafafa",
      "editor.lineHighlightBackground": "#18181b",
      "editorUnnecessaryCode.opacity": "#00000099",
    },
  });

  monacoModule.editor.defineTheme("vizsla-lab-light", {
    base: "vs",
    inherit: true,
    rules: [
      { token: "keyword", foreground: "2563eb" },
      { token: "keyword.preprocessor", foreground: "b45309" },
      { token: "type.identifier", foreground: "0f766e" },
      { token: "number", foreground: "dc2626" },
      ...semanticModifierRules,
    ],
    colors: {
      "editor.background": "#ffffff",
      "editor.foreground": "#18181b",
      "editorLineNumber.foreground": "#a1a1aa",
      "editorCursor.foreground": "#09090b",
      "editor.selectionBackground": "#d4d4d8",
      "editor.inactiveSelectionBackground": "#e4e4e7",
      "editorGutter.background": "#ffffff",
      "editorLineNumber.activeForeground": "#18181b",
      "editor.lineHighlightBackground": "#f4f4f5",
      "editorUnnecessaryCode.opacity": "#00000099",
    },
  });
}

function semanticTokenTypesFromCapabilities(serverCapabilities: unknown): string[] {
  if (!isRecord(serverCapabilities) || !isRecord(serverCapabilities.semanticTokensProvider)) {
    return [];
  }
  const legend = serverCapabilities.semanticTokensProvider.legend;
  if (!isRecord(legend) || !Array.isArray(legend.tokenTypes)) {
    return [];
  }
  return legend.tokenTypes.filter((item): item is string => typeof item === "string");
}

export async function wireVizslaVscodeLanguage(
  _editor: monaco.editor.IStandaloneCodeEditor,
): Promise<void> {
  await Promise.all([
    applyLanguageConfiguration("systemverilog", languageConfiguration),
    applyLanguageConfiguration("verilog", languageConfiguration),
  ]);

  const registry = new Registry({
    onigLib: getOnigLib(),
    loadGrammar: (scopeName: string) => {
      const grammar = grammarForScope(scopeName);
      if (!grammar) {
        return Promise.resolve(null);
      }
      return Promise.resolve(parseRawGrammar(grammar.source, grammar.path));
    },
  });

  await wireVizslaTextMateGrammars(registry, new Map([["verilog", "source.verilog"], ["systemverilog", "source.systemverilog"]]));
}

function applyLanguageConfiguration(languageId: string, raw: VscodeLanguageConfiguration): void {
  monaco.languages.setLanguageConfiguration(languageId, toMonacoLanguageConfiguration(raw));
}

function grammarForScope(scopeName: string): { path: string; source: string } | null {
  switch (scopeName) {
    case "source.verilog":
      return { path: "verilog.tmLanguage.json", source: verilogGrammar };
    case "source.systemverilog":
      return { path: "systemverilog.tmLanguage.json", source: systemVerilogGrammar };
    default:
      return null;
  }
}

function getOnigLib(): Promise<IOnigLib> {
  onigLibPromise ??= fetch(onigWasm)
    .then((response) => response.arrayBuffer())
    .then(async (wasm) => {
      await loadWASM(wasm);
      return {
        createOnigScanner(patterns) {
          return createOnigScanner(patterns) as unknown as ReturnType<IOnigLib["createOnigScanner"]>;
        },
        createOnigString(value) {
          return createOnigString(value) as unknown as ReturnType<IOnigLib["createOnigString"]>;
        },
      };
    });
  return onigLibPromise;
}

async function wireVizslaTextMateGrammars(registry: Registry, grammars: Map<string, string>): Promise<void> {
  await Promise.all(
    Array.from(grammars, async ([languageId, scopeName]) => {
      const grammar = await registry.loadGrammar(scopeName);
      if (!grammar) {
        throw new Error(`Missing TextMate grammar for ${scopeName}`);
      }

      monaco.languages.setTokensProvider(languageId, {
        getInitialState: () => new TextMateState(INITIAL),
        tokenize: (line, state) => {
          const currentState = state instanceof TextMateState ? state.ruleStack : INITIAL;
          const result = grammar.tokenizeLine(line, currentState);
          return {
            endState: new TextMateState(result.ruleStack),
            tokens: result.tokens.map((token) => ({
              startIndex: token.startIndex,
              scopes: toMonacoToken(token.scopes),
            })),
          };
        },
      });
    }),
  );
}

class TextMateState implements monaco.languages.IState {
  constructor(readonly ruleStack: StateStack) {}

  clone(): TextMateState {
    return new TextMateState(this.ruleStack.clone());
  }

  equals(other: monaco.languages.IState): boolean {
    return other instanceof TextMateState && this.ruleStack.equals(other.ruleStack);
  }
}

function toMonacoToken(scopes: string[]): string {
  for (let index = scopes.length - 1; index >= 0; index -= 1) {
    const scope = scopes[index];
    if (!scope.startsWith("source.")) {
      return scope;
    }
  }
  return "";
}

interface VscodeLanguageConfiguration {
  comments?: {
    lineComment?: string | { comment: string; noIndent?: boolean };
    blockComment?: readonly string[];
  };
  brackets?: readonly (readonly string[])[];
  autoClosingPairs?: Array<{ open: string; close: string; notIn?: string[] }>;
  surroundingPairs?: readonly (readonly string[])[];
  folding?: {
    markers?: {
      start: string;
      end: string;
    };
  };
  wordPattern?: string;
  indentationRules?: {
    increaseIndentPattern?: string;
    decreaseIndentPattern?: string;
    indentNextLinePattern?: string;
    unIndentedLinePattern?: string;
  };
  onEnterRules?: Array<{
    beforeText: string;
    afterText?: string;
    previousLineText?: string;
    action: {
      indent: string;
      appendText?: string;
      removeText?: number;
    };
  }>;
}

function toMonacoLanguageConfiguration(raw: VscodeLanguageConfiguration): monaco.languages.LanguageConfiguration {
  return {
    comments: raw.comments
      ? {
          lineComment:
            typeof raw.comments.lineComment === "string" ? raw.comments.lineComment : raw.comments.lineComment?.comment,
          blockComment: toCharacterPair(raw.comments.blockComment),
        }
      : undefined,
    brackets: raw.brackets?.flatMap((pair) => {
      const characterPair = toCharacterPair(pair);
      return characterPair ? [characterPair] : [];
    }),
    autoClosingPairs: raw.autoClosingPairs,
    surroundingPairs: raw.surroundingPairs?.flatMap((pair) => {
      const characterPair = toCharacterPair(pair);
      return characterPair ? [{ open: characterPair[0], close: characterPair[1] }] : [];
    }),
    folding: raw.folding?.markers
      ? {
          markers: {
            start: new RegExp(raw.folding.markers.start),
            end: new RegExp(raw.folding.markers.end),
          },
        }
      : undefined,
    wordPattern: raw.wordPattern ? new RegExp(raw.wordPattern) : undefined,
    indentationRules: toIndentationRules(raw.indentationRules),
    onEnterRules: raw.onEnterRules?.map((rule) => ({
      beforeText: new RegExp(rule.beforeText),
      afterText: regexOrUndefined(rule.afterText),
      previousLineText: regexOrUndefined(rule.previousLineText),
      action: {
        indentAction: indentAction(rule.action.indent),
        appendText: rule.action.appendText,
        removeText: rule.action.removeText,
      },
    })),
  };
}

function toCharacterPair(pair: readonly string[] | undefined): [string, string] | undefined {
  if (pair?.length === 2) {
    return [pair[0], pair[1]];
  }
  return undefined;
}

function toIndentationRules(
  raw: VscodeLanguageConfiguration["indentationRules"],
): monaco.languages.IndentationRule | undefined {
  const increaseIndentPattern = regexOrUndefined(raw?.increaseIndentPattern);
  const decreaseIndentPattern = regexOrUndefined(raw?.decreaseIndentPattern);
  if (!increaseIndentPattern || !decreaseIndentPattern) {
    return undefined;
  }

  return {
    increaseIndentPattern,
    decreaseIndentPattern,
    indentNextLinePattern: regexOrUndefined(raw?.indentNextLinePattern) ?? null,
    unIndentedLinePattern: regexOrUndefined(raw?.unIndentedLinePattern) ?? null,
  };
}

function regexOrUndefined(pattern: string | undefined): RegExp | undefined {
  return pattern ? new RegExp(pattern) : undefined;
}

function indentAction(value: string): monaco.languages.IndentAction {
  switch (value) {
    case "indent":
      return monaco.languages.IndentAction.Indent;
    case "indentOutdent":
      return monaco.languages.IndentAction.IndentOutdent;
    case "outdent":
      return monaco.languages.IndentAction.Outdent;
    default:
      return monaco.languages.IndentAction.None;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function installShadowCaretRangeFromPoint(): void {
  if (!globalThis.ShadowRoot || typeof globalThis.document === "undefined") {
    return;
  }

  const prototype = globalThis.ShadowRoot.prototype as ShadowRoot & {
    caretRangeFromPoint?: (x: number, y: number) => Range | null;
  };
  if (typeof prototype.caretRangeFromPoint === "function") {
    return;
  }

  Object.defineProperty(prototype, "caretRangeFromPoint", {
    configurable: true,
    value(this: ShadowRoot, x: number, y: number): Range | null {
      const target = shadowElementFromPoint(this, x, y);
      const textNode = target ? nearestTextNode(target) : null;
      const range = document.createRange();
      if (!textNode) {
        range.selectNodeContents(this.host);
        range.collapse(false);
        return range;
      }

      const offset = textOffsetAtX(textNode, x);
      range.setStart(textNode, offset);
      range.setEnd(textNode, offset);
      return range;
    },
  });
}

function shadowElementFromPoint(root: ShadowRoot, x: number, y: number): Element | null {
  const nativeElementFromPoint = (root as ShadowRoot & {
    elementFromPoint?: (x: number, y: number) => Element | null;
  }).elementFromPoint;
  if (typeof nativeElementFromPoint === "function") {
    return nativeElementFromPoint.call(root, x, y);
  }

  let match: Element | null = null;
  let matchArea = Number.POSITIVE_INFINITY;
  for (const element of root.querySelectorAll("*")) {
    const rect = element.getBoundingClientRect();
    if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) {
      continue;
    }

    const area = rect.width * rect.height;
    if (area <= matchArea) {
      match = element;
      matchArea = area;
    }
  }
  return match;
}

function nearestTextNode(element: Element): Text | null {
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  let current = walker.nextNode();
  let candidate: Text | null = null;
  while (current) {
    if (current.textContent && current.textContent.length > 0) {
      candidate = current as Text;
    }
    current = walker.nextNode();
  }
  return candidate;
}

function textOffsetAtX(textNode: Text, x: number): number {
  const text = textNode.data;
  if (text.length === 0) {
    return 0;
  }

  const parent = textNode.parentElement;
  if (!parent) {
    return text.length;
  }

  const rect = parent.getBoundingClientRect();
  if (rect.width <= 0) {
    return text.length;
  }
  if (x <= rect.left) {
    return 0;
  }
  if (x >= rect.right) {
    return text.length;
  }

  return Math.max(0, Math.min(text.length, Math.round(((x - rect.left) / rect.width) * text.length)));
}
