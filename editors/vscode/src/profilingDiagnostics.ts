import type { LspMessage } from './profilingTypes';

export type DiagnosticProfileRequest = 'workspace/diagnostic' | 'textDocument/diagnostic';

export function diagnosticsFromProfileResponse(
  response: LspMessage,
  request: DiagnosticProfileRequest,
): LspMessage[] {
  return request === 'workspace/diagnostic'
    ? diagnosticsFromWorkspaceResponse(response)
    : diagnosticsFromDocumentResponse(response);
}

export function diagnosticsFromDocumentResponse(response: LspMessage): LspMessage[] {
  const result = asObject(response.result);
  const items = Array.isArray(result?.items) ? result.items : [];
  return items.filter(isObject);
}

export function diagnosticsFromWorkspaceResponse(response: LspMessage): LspMessage[] {
  const result = asObject(response.result);
  const reports = Array.isArray(result?.items) ? result.items : [];
  const diagnostics: LspMessage[] = [];

  for (const report of reports) {
    const reportObject = asObject(report);
    const items = Array.isArray(reportObject?.items) ? reportObject.items : [];
    diagnostics.push(...items.filter(isObject));
  }

  return diagnostics;
}

export function summarizeDiagnostics(diagnostics: LspMessage[]): Record<string, unknown> {
  const severities = new Map<string, number>();
  const sources = new Map<string, number>();
  const codes = new Map<string, number>();
  const messages = new Map<string, number>();

  for (const diagnostic of diagnostics) {
    count(severities, diagnostic.severity === undefined ? 'none' : String(diagnostic.severity));
    count(sources, typeof diagnostic.source === 'string' ? diagnostic.source : 'none');
    count(codes, diagnosticCode(diagnostic));
    const firstLine = String(diagnostic.message ?? '').split(/\r?\n/, 1)[0] ?? '';
    count(messages, firstLine.slice(0, 180));
  }

  return {
    diagnostic_count: diagnostics.length,
    severity: topCounts(severities, Number.MAX_SAFE_INTEGER),
    sources: topCounts(sources, 20),
    codes: topCounts(codes, 20),
    top_messages: topCounts(messages, 20),
  };
}

function diagnosticCode(diagnostic: LspMessage): string {
  if (isObject(diagnostic.code)) {
    return String(diagnostic.code.value ?? 'none');
  }
  return diagnostic.code === undefined ? 'none' : String(diagnostic.code);
}

function count(map: Map<string, number>, key: string): void {
  map.set(key, (map.get(key) ?? 0) + 1);
}

function topCounts(map: Map<string, number>, limit: number): [string, number][] {
  return [...map.entries()]
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]))
    .slice(0, limit);
}

function isObject(value: unknown): value is LspMessage {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function asObject(value: unknown): LspMessage | undefined {
  return isObject(value) ? value : undefined;
}
