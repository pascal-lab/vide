import type * as Monaco from "@codingame/monaco-vscode-editor-api";
import type { LabDiagnostic, LspPosition, LspRange, LspSeverity } from "../types";

type MonacoModule = typeof Monaco;

export function diagnosticsFromLspReport(result: unknown, uri: string, filePath: string): LabDiagnostic[] {
  if (!isRecord(result)) {
    return [];
  }
  const items = Array.isArray(result.items) ? result.items : [];
  return items.filter(isLspDiagnostic).map((diagnostic) => {
    const data = isRecord(diagnostic.data) ? diagnostic.data : {};
    const diagnosticName = stringValue(data.name);
    const source = stringValue(diagnostic.source) ?? "vide";
    const rawCode = lspCodeToString(diagnostic.code);
    return {
      uri,
      filePath,
      range: diagnostic.range,
      severity: diagnostic.severity ?? 3,
      tags: diagnosticTags(diagnostic.tags),
      source,
      title: diagnosticName ?? source,
      code: diagnosticName ?? rawCode,
      rawCode,
      data: diagnostic.data,
      message: diagnostic.message,
    };
  });
}

export function toMarkerData(monaco: MonacoModule, diagnostic: LabDiagnostic): Monaco.editor.IMarkerData {
  return {
    severity: markerSeverity(monaco, diagnostic.severity),
    message: diagnostic.message,
    code: diagnostic.code,
    source: diagnostic.source,
    tags: markerTags(monaco, diagnostic.tags),
    startLineNumber: diagnostic.range.start.line + 1,
    startColumn: diagnostic.range.start.character + 1,
    endLineNumber: diagnostic.range.end.line + 1,
    endColumn: diagnostic.range.end.character + 1,
  };
}

function markerSeverity(monaco: MonacoModule, severity: LspSeverity): Monaco.MarkerSeverity {
  switch (severity) {
    case 1:
      return monaco.MarkerSeverity.Error;
    case 2:
      return monaco.MarkerSeverity.Warning;
    case 3:
      return monaco.MarkerSeverity.Info;
    default:
      return monaco.MarkerSeverity.Hint;
  }
}

function markerTags(monaco: MonacoModule, tags: readonly number[] | undefined): Monaco.MarkerTag[] | undefined {
  const markerTags: Monaco.MarkerTag[] = [];
  if (tags?.includes(1)) {
    markerTags.push(monaco.MarkerTag.Unnecessary);
  }
  if (tags?.includes(2)) {
    markerTags.push(monaco.MarkerTag.Deprecated);
  }
  return markerTags.length > 0 ? markerTags : undefined;
}

function isLspDiagnostic(
  value: unknown,
): value is {
  range: LspRange;
  severity?: LspSeverity;
  tags?: unknown;
  source?: unknown;
  code?: unknown;
  data?: unknown;
  message: string;
} {
  return isRecord(value) && isLspRange(value.range) && typeof value.message === "string";
}

function isLspRange(value: unknown): value is LspRange {
  return isRecord(value) && isLspPosition(value.start) && isLspPosition(value.end);
}

function isLspPosition(value: unknown): value is LspPosition {
  return isRecord(value) && typeof value.line === "number" && typeof value.character === "number";
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function diagnosticTags(tags: unknown): number[] | undefined {
  if (!Array.isArray(tags)) {
    return undefined;
  }
  const result = tags.filter((tag): tag is number => tag === 1 || tag === 2);
  return result.length > 0 ? result : undefined;
}

function lspCodeToString(code: unknown): string | undefined {
  if (typeof code === "string" || typeof code === "number") {
    return String(code);
  }
  return undefined;
}
