import * as fs from 'node:fs';
import * as path from 'node:path';

import type { LspMessage } from './profilingTypes';

type Frame = {
  name: string;
  startUs: number;
  childUs: number;
};

type SpanAggregate = {
  count: number;
  totalMs: number;
  maxMs: number;
};

type TraceSummary = {
  eventCount: number;
  threadCount: number;
  spans: Map<string, SpanAggregate>;
  folded: Map<string, number>;
};

type FlameNode = {
  name: string;
  value: number;
  children: Map<string, FlameNode>;
};

export async function summarizeTraceFile(
  tracePath: string,
  foldedPath: string,
  svgPath: string,
): Promise<Record<string, unknown>> {
  const text = await fs.promises.readFile(tracePath, 'utf8');
  const parsed = JSON.parse(text) as unknown;
  const summary = summarizeTraceEvents(traceEvents(parsed), 100);
  await writeFoldedFile(foldedPath, summary.folded);
  await writeFlamegraphSvg(foldedPath, svgPath, path.basename(tracePath));
  return traceSummaryJson(summary, 30);
}

export function traceSummaryFromJson(
  parsed: unknown,
  options?: { minUs?: number; top?: number },
): Record<string, unknown> {
  const summary = summarizeTraceEvents(traceEvents(parsed), options?.minUs ?? 100);
  return traceSummaryJson(summary, options?.top ?? 30);
}

export function foldedStacksFromJson(parsed: unknown, minUs = 100): Map<string, number> {
  return summarizeTraceEvents(traceEvents(parsed), minUs).folded;
}

function traceEvents(parsed: unknown): LspMessage[] {
  if (Array.isArray(parsed)) {
    return parsed.filter(isObject);
  }
  const object = asObject(parsed);
  return Array.isArray(object?.traceEvents) ? object.traceEvents.filter(isObject) : [];
}

function summarizeTraceEvents(events: LspMessage[], minUs: number): TraceSummary {
  const threadNames = collectThreadNames(events);
  const stacks = new Map<string, Frame[]>();
  const summary: TraceSummary = {
    eventCount: events.length,
    threadCount: 0,
    spans: new Map(),
    folded: new Map(),
  };

  for (const event of events) {
    const phase = typeof event.ph === 'string' ? event.ph : undefined;
    if (!phase || !['B', 'E', 'X'].includes(phase)) {
      continue;
    }
    const key = eventThreadKey(event);
    const timestamp = numberValue(event.ts);
    const name = typeof event.name === 'string' ? event.name : 'unknown';
    const stack = getOrInsert(stacks, key, () => []);

    if (phase === 'B') {
      stack.push({ name, startUs: timestamp, childUs: 0 });
    } else if (phase === 'E') {
      const frame = stack.pop();
      if (!frame) {
        continue;
      }
      const durationUs = Math.max(0, timestamp - frame.startUs);
      const parent = stack.at(-1);
      if (parent) {
        parent.childUs += durationUs;
      }
      recordSpan(summary, threadNames, key, stack, frame.name, durationUs, frame.childUs, minUs);
    } else {
      const durationUs = Math.max(0, numberValue(event.dur));
      recordSpan(summary, threadNames, key, stack, name, durationUs, 0, minUs);
    }
  }

  summary.threadCount = Math.max(stacks.size, threadNames.size);
  return summary;
}

function traceSummaryJson(summary: TraceSummary, top: number): Record<string, unknown> {
  return {
    event_count: summary.eventCount,
    thread_count: summary.threadCount,
    top_by_total_ms: topSpanRows(summary.spans, top, true),
    top_by_max_ms: topSpanRows(summary.spans, top, false),
  };
}

function collectThreadNames(events: LspMessage[]): Map<string, string> {
  const names = new Map<string, string>();
  for (const event of events) {
    if (event.ph !== 'M' || event.name !== 'thread_name') {
      continue;
    }
    const args = asObject(event.args);
    names.set(eventThreadKey(event), typeof args?.name === 'string' ? args.name : 'thread');
  }
  return names;
}

function eventThreadKey(event: LspMessage): string {
  return `${numberValue(event.pid)}:${numberValue(event.tid)}`;
}

function recordSpan(
  summary: TraceSummary,
  threadNames: Map<string, string>,
  key: string,
  stack: Frame[],
  name: string,
  durationUs: number,
  childUs: number,
  minUs: number,
): void {
  const durationMs = durationUs / 1000;
  const aggregate = getOrInsert(summary.spans, name, () => ({ count: 0, totalMs: 0, maxMs: 0 }));
  aggregate.count += 1;
  aggregate.totalMs += durationMs;
  aggregate.maxMs = Math.max(aggregate.maxMs, durationMs);

  const selfUs = Math.max(0, durationUs - childUs);
  if (selfUs >= minUs) {
    const frames = [
      sanitizeFrameName(threadNames.get(key) ?? 'thread'),
      ...stack.map((frame) => sanitizeFrameName(frame.name)),
      sanitizeFrameName(name),
    ];
    const folded = frames.join(';');
    summary.folded.set(folded, (summary.folded.get(folded) ?? 0) + selfUs);
  }
}

function sanitizeFrameName(name: string): string {
  return name.replace(/;/g, ',').replace(/\r?\n/g, ' ');
}

function topSpanRows(
  spans: Map<string, SpanAggregate>,
  limit: number,
  byTotal: boolean,
): Record<string, unknown>[] {
  const rows = [...spans.entries()].map(([name, aggregate]) => ({
    name,
    count: aggregate.count,
    total_ms: aggregate.totalMs,
    max_ms: aggregate.maxMs,
    mean_ms: aggregate.totalMs / aggregate.count,
  }));
  rows.sort((left, right) =>
    byTotal ? right.total_ms - left.total_ms : right.max_ms - left.max_ms,
  );
  return rows.slice(0, limit);
}

async function writeFoldedFile(foldedPath: string, folded: Map<string, number>): Promise<void> {
  const lines = [...folded.entries()].map(([stack, value]) => `${stack} ${value.toFixed(0)}`);
  await fs.promises.writeFile(foldedPath, `${lines.join('\n')}\n`, 'utf8');
}

async function writeFlamegraphSvg(
  foldedPath: string,
  svgPath: string,
  title: string,
): Promise<void> {
  const root = await flameRootFromFoldedFile(foldedPath);
  const output = renderFlamegraphSvg(root, title);
  await fs.promises.writeFile(svgPath, `${output.join('\n')}\n`, 'utf8');
}

async function flameRootFromFoldedFile(foldedPath: string): Promise<FlameNode> {
  const folded = await fs.promises.readFile(foldedPath, 'utf8');
  const root: FlameNode = { name: 'all', value: 0, children: new Map() };
  for (const line of folded.split(/\r?\n/).filter((item) => item.trim().length > 0)) {
    const index = line.lastIndexOf(' ');
    if (index < 0) {
      continue;
    }
    const stack = line.slice(0, index).split(';');
    const value = Number(line.slice(index + 1));
    if (Number.isFinite(value)) {
      insertFlameStack(root, stack, value);
    }
  }
  return root;
}

function renderFlamegraphSvg(root: FlameNode, title: string): string[] {
  const width = 1400;
  const marginLeft = 10;
  const marginTop = 38;
  const marginBottom = 28;
  const frameHeight = 17;
  const graphWidth = width - marginLeft * 2;
  const depth = flameDepth(root);
  const height = marginTop + marginBottom + depth * frameHeight;
  const scale = root.value > 0 ? graphWidth / root.value : 1;
  const output: string[] = [];

  output.push('<?xml version="1.0" encoding="UTF-8"?>');
  output.push(
    `<svg version="1.1" width="${width}" height="${height}" xmlns="http://www.w3.org/2000/svg">`,
  );
  output.push(
    '<style>text{font-family:Verdana,Arial,sans-serif;font-size:11px}.title{font-size:16px;font-weight:700}.frame:hover{stroke:#000;stroke-width:0.7}</style>',
  );
  output.push('<rect width="100%" height="100%" fill="#fff"/>');
  output.push(`<text class="title" x="${marginLeft}" y="22">${escapeXml(title)}</text>`);
  renderFlameNode(root, marginLeft, 0, height, marginBottom, frameHeight, scale, output);
  output.push('</svg>');
  return output;
}

function insertFlameStack(node: FlameNode, stack: string[], value: number): void {
  node.value += value;
  const [head, ...tail] = stack;
  if (!head) {
    return;
  }
  const child = getOrInsert(node.children, head, () => ({
    name: head,
    value: 0,
    children: new Map(),
  }));
  insertFlameStack(child, tail, value);
}

function flameDepth(node: FlameNode): number {
  const childDepth = [...node.children.values()].reduce(
    (maxDepth, child) => Math.max(maxDepth, flameDepth(child)),
    0,
  );
  return 1 + childDepth;
}

function renderFlameNode(
  node: FlameNode,
  x: number,
  depth: number,
  height: number,
  marginBottom: number,
  frameHeight: number,
  scale: number,
  output: string[],
): void {
  const children = [...node.children.values()].sort((left, right) => right.value - left.value);
  let cursor = x;
  const y = height - marginBottom - (depth + 1) * frameHeight;

  for (const child of children) {
    const rectWidth = child.value * scale;
    if (rectWidth < 0.5) {
      cursor += rectWidth;
      continue;
    }
    const name = escapeXml(child.name);
    const ms = child.value / 1000;
    output.push(
      `<g><title>${name} (${ms.toFixed(3)} ms)</title><rect class="frame" x="${cursor.toFixed(3)}" y="${y.toFixed(3)}" width="${rectWidth.toFixed(3)}" height="${(frameHeight - 1).toFixed(3)}" fill="${flameColor(child.name)}" stroke="#fff" stroke-width="0.3"/>`,
    );
    if (rectWidth > 24) {
      const maxChars = Math.max(1, Math.floor(rectWidth / 7));
      const label = truncateLabel(child.name, maxChars);
      output.push(
        `<text x="${(cursor + 3).toFixed(3)}" y="${(y + 12).toFixed(3)}" fill="#111">${escapeXml(label)}</text>`,
      );
    }
    output.push('</g>');
    renderFlameNode(child, cursor, depth + 1, height, marginBottom, frameHeight, scale, output);
    cursor += rectWidth;
  }
}

function truncateLabel(label: string, maxChars: number): string {
  if ([...label].length <= maxChars) {
    return label;
  }
  const keep = Math.max(0, maxChars - 3);
  return `${[...label].slice(0, keep).join('')}...`;
}

function flameColor(name: string): string {
  const hash = [...Buffer.from(name)].reduce(
    (acc, byte, index) => (acc + (index + 1) * byte) >>> 0,
    0,
  );
  return `rgb(${180 + (hash % 60)},${80 + (Math.floor(hash / 7) % 90)},${40 + (Math.floor(hash / 13) % 60)})`;
}

function numberValue(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

function isObject(value: unknown): value is LspMessage {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function asObject(value: unknown): LspMessage | undefined {
  return isObject(value) ? value : undefined;
}

function getOrInsert<K, V>(map: Map<K, V>, key: K, create: () => V): V {
  const existing = map.get(key);
  if (existing !== undefined) {
    return existing;
  }
  const value = create();
  map.set(key, value);
  return value;
}

function escapeXml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
