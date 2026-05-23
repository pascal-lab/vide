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

type FlameNodeJson = {
  name: string;
  value: number;
  children: FlameNodeJson[];
};

export async function summarizeTraceFile(
  tracePath: string,
  foldedPath: string,
  svgPath: string,
  htmlPath: string,
): Promise<Record<string, unknown>> {
  const text = await fs.promises.readFile(tracePath, 'utf8');
  const parsed = JSON.parse(text) as unknown;
  const summary = summarizeTraceEvents(traceEvents(parsed), 100);
  await writeFoldedFile(foldedPath, summary.folded);
  await writeFlamegraphSvg(foldedPath, svgPath, path.basename(tracePath));
  await writeFlamegraphHtml(foldedPath, htmlPath, path.basename(tracePath));
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

async function writeFlamegraphHtml(
  foldedPath: string,
  htmlPath: string,
  title: string,
): Promise<void> {
  const root = await flameRootFromFoldedFile(foldedPath);
  await fs.promises.writeFile(htmlPath, flamegraphHtml(root, title), 'utf8');
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

function flamegraphHtml(root: FlameNode, title: string): string {
  const data = JSON.stringify(flameNodeJson(root)).replace(/</g, '\\u003c');
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${escapeHtml(title)} flamegraph</title>
<style>
:root{color-scheme:light dark;font-family:Inter,Segoe UI,Arial,sans-serif;background:#f7f7f4;color:#161616}
body{margin:0}
header{display:flex;gap:12px;align-items:center;justify-content:space-between;padding:12px 16px;border-bottom:1px solid #d7d4cc;background:#fff}
h1{font-size:16px;margin:0;font-weight:650}
.controls{display:flex;gap:8px;align-items:center}
button,input{font:inherit;border:1px solid #bbb7ae;border-radius:4px;background:#fff;color:#161616;padding:6px 9px}
input{width:260px}
main{height:calc(100vh - 58px);overflow:auto;background:#fff}
#chart{display:block;min-width:1200px}
.frame{cursor:pointer;stroke:#fff;stroke-width:.5}
.frame:hover{stroke:#111;stroke-width:1}
.hit{stroke:#111;stroke-width:1.5}
text{font-size:11px;pointer-events:none;fill:#111}
.crumbs{font-size:12px;color:#5d574d;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
@media (prefers-color-scheme:dark){:root{background:#171717;color:#f3f0ea}header,main,button,input{background:#202020;color:#f3f0ea;border-color:#4a4740}text{fill:#101010}.crumbs{color:#b9b2a6}}
</style>
</head>
<body>
<header>
  <div>
    <h1>${escapeHtml(title)}</h1>
    <div class="crumbs" id="crumbs">all</div>
  </div>
  <div class="controls">
    <input id="search" type="search" placeholder="Search frames">
    <button id="reset" type="button">Reset</button>
  </div>
</header>
<main><svg id="chart" role="img" aria-label="Interactive flamegraph"></svg></main>
<script>
const root = ${data};
const chart = document.getElementById('chart');
const crumbs = document.getElementById('crumbs');
const search = document.getElementById('search');
const reset = document.getElementById('reset');
let current = root;
let path = [root.name];
const width = 1400;
const frameHeight = 18;
const topPad = 8;
const bottomPad = 18;
function color(name){
  let hash = 0;
  for (let i = 0; i < name.length; i++) hash = (hash + (i + 1) * name.charCodeAt(i)) >>> 0;
  return 'rgb(' + (180 + hash % 60) + ',' + (80 + Math.floor(hash / 7) % 90) + ',' + (40 + Math.floor(hash / 13) % 60) + ')';
}
function depth(node){
  return 1 + node.children.reduce((max, child) => Math.max(max, depth(child)), 0);
}
function label(text, rectWidth){
  const max = Math.floor(rectWidth / 7);
  if (max <= 3) return '';
  return text.length > max ? text.slice(0, max - 3) + '...' : text;
}
function render(){
  const height = topPad + bottomPad + depth(current) * frameHeight;
  const scale = current.value > 0 ? (width - 20) / current.value : 1;
  const query = search.value.trim().toLowerCase();
  chart.setAttribute('width', String(width));
  chart.setAttribute('height', String(height));
  chart.setAttribute('viewBox', '0 0 ' + width + ' ' + height);
  chart.innerHTML = '';
  crumbs.textContent = path.join(' / ');
  drawChildren(current, 10, 0, scale, height, query);
}
function drawChildren(node, startX, level, scale, height, query){
  const y = height - bottomPad - (level + 1) * frameHeight;
  let x = startX;
  for (const child of [...node.children].sort((a,b) => b.value - a.value || a.name.localeCompare(b.name))) {
    const rectWidth = child.value * scale;
    if (rectWidth < .5) { x += rectWidth; continue; }
    const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
    title.textContent = child.name + ' (' + (child.value / 1000).toFixed(3) + ' ms)';
    const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
    rect.setAttribute('class', 'frame' + (query && child.name.toLowerCase().includes(query) ? ' hit' : ''));
    rect.setAttribute('x', x.toFixed(3));
    rect.setAttribute('y', y.toFixed(3));
    rect.setAttribute('width', rectWidth.toFixed(3));
    rect.setAttribute('height', String(frameHeight - 1));
    rect.setAttribute('fill', color(child.name));
    group.append(title, rect);
    const visibleLabel = label(child.name, rectWidth);
    if (visibleLabel) {
      const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
      text.setAttribute('x', String(x + 3));
      text.setAttribute('y', String(y + 12));
      text.textContent = visibleLabel;
      group.append(text);
    }
    group.addEventListener('click', (event) => {
      event.stopPropagation();
      current = child;
      path.push(child.name);
      render();
    });
    chart.append(group);
    drawChildren(child, x, level + 1, scale, height, query);
    x += rectWidth;
  }
}
reset.addEventListener('click', () => { current = root; path = [root.name]; render(); });
search.addEventListener('input', render);
chart.addEventListener('click', () => { if (path.length > 1) { path.pop(); current = findByPath(root, path.slice(1)); render(); } });
function findByPath(node, names){
  let cursor = node;
  for (const name of names) cursor = cursor.children.find((child) => child.name === name) || cursor;
  return cursor;
}
render();
</script>
</body>
</html>
`;
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

function flameNodeJson(node: FlameNode): FlameNodeJson {
  return {
    name: node.name,
    value: node.value,
    children: [...node.children.values()].map(flameNodeJson),
  };
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

function escapeHtml(text: string): string {
  return escapeXml(text).replace(/'/g, '&#39;');
}
