import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import * as os from 'node:os';
import * as path from 'node:path';

import { stripProfileArgs } from '../src/profilingArgs';
import {
  diagnosticsFromDocumentResponse,
  diagnosticsFromProfileResponse,
  diagnosticsFromWorkspaceResponse,
  summarizeDiagnostics,
} from '../src/profilingDiagnostics';
import {
  foldedStacksFromJson,
  summarizeTraceFile,
  traceSummaryFromJson,
} from '../src/profilingTrace';

test('removes profiling-owned server arguments before launching a profile session', () => {
  assert.deepEqual(
    stripProfileArgs([
      '--foo',
      'bar',
      '--log',
      'debug',
      '--log_file=old.log',
      '--profile_trace',
      'old.json',
      '--profile_trace=older.json',
      '--baz',
    ]),
    ['--foo', 'bar', '--baz'],
  );
});

test('summarizes document diagnostics defensively', () => {
  const diagnostics = diagnosticsFromDocumentResponse({
    result: {
      items: [
        {
          severity: 1,
          source: 'slang',
          code: { value: '6:129' },
          message: 'first line\nsecond line',
        },
        {
          severity: 2,
          source: 'vizsla',
          code: 'missing-manifest',
          message: 'Project manifest missing',
        },
      ],
    },
  });

  assert.equal(diagnostics.length, 2);
  assert.deepEqual(summarizeDiagnostics(diagnostics), {
    diagnostic_count: 2,
    severity: [
      ['1', 1],
      ['2', 1],
    ],
    sources: [
      ['slang', 1],
      ['vizsla', 1],
    ],
    codes: [
      ['6:129', 1],
      ['missing-manifest', 1],
    ],
    top_messages: [
      ['first line', 1],
      ['Project manifest missing', 1],
    ],
  });
});

test('summarizes workspace diagnostics defensively', () => {
  const response = {
    result: {
      items: [
        {
          kind: 'full',
          uri: 'file:///workspace/a.sv',
          items: [
            {
              severity: 1,
              source: 'vizsla',
              code: 'missing-manifest',
              message: 'Project manifest missing',
            },
          ],
        },
        {
          kind: 'unchanged',
          uri: 'file:///workspace/b.sv',
          resultId: 'previous',
        },
        {
          kind: 'full',
          uri: 'file:///workspace/c.sv',
          items: [
            {
              severity: 2,
              source: 'slang',
              code: { value: '6:129' },
              message: 'first line\nsecond line',
            },
          ],
        },
      ],
    },
  };

  const diagnostics = diagnosticsFromWorkspaceResponse(response);

  assert.deepEqual(diagnosticsFromProfileResponse(response, 'workspace/diagnostic'), diagnostics);
  assert.equal(diagnostics.length, 2);
  assert.deepEqual(summarizeDiagnostics(diagnostics), {
    diagnostic_count: 2,
    severity: [
      ['1', 1],
      ['2', 1],
    ],
    sources: [
      ['slang', 1],
      ['vizsla', 1],
    ],
    codes: [
      ['6:129', 1],
      ['missing-manifest', 1],
    ],
    top_messages: [
      ['first line', 1],
      ['Project manifest missing', 1],
    ],
  });
});

test('summarizes chrome trace spans and folded self time', () => {
  const trace = {
    traceEvents: [
      { ph: 'M', name: 'thread_name', pid: 1, tid: 7, args: { name: 'worker' } },
      { ph: 'B', name: 'outer', pid: 1, tid: 7, ts: 0 },
      { ph: 'B', name: 'inner', pid: 1, tid: 7, ts: 100 },
      { ph: 'E', name: 'inner', pid: 1, tid: 7, ts: 300 },
      { ph: 'E', name: 'outer', pid: 1, tid: 7, ts: 500 },
      { ph: 'X', name: 'instant-like', pid: 1, tid: 8, ts: 10, dur: 250 },
    ],
  };

  assert.deepEqual(traceSummaryFromJson(trace, { minUs: 50, top: 2 }), {
    event_count: 6,
    thread_count: 2,
    top_by_total_ms: [
      { name: 'outer', count: 1, total_ms: 0.5, max_ms: 0.5, mean_ms: 0.5 },
      { name: 'instant-like', count: 1, total_ms: 0.25, max_ms: 0.25, mean_ms: 0.25 },
    ],
    top_by_max_ms: [
      { name: 'outer', count: 1, total_ms: 0.5, max_ms: 0.5, mean_ms: 0.5 },
      { name: 'instant-like', count: 1, total_ms: 0.25, max_ms: 0.25, mean_ms: 0.25 },
    ],
  });

  assert.deepEqual([...foldedStacksFromJson(trace, 50).entries()], [
    ['worker;outer;inner', 200],
    ['worker;outer', 300],
    ['thread;instant-like', 250],
  ]);
});

test('writes trace summary, folded stacks, and flamegraph artifacts', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'vizsla-profile-test-'));
  const trace = path.join(dir, 'trace.json');
  const folded = path.join(dir, 'trace.folded');
  const svg = path.join(dir, 'flamegraph.svg');
  const html = path.join(dir, 'flamegraph.html');

  await fs.writeFile(
    trace,
    JSON.stringify({
      traceEvents: [
        { ph: 'B', name: 'outer', pid: 1, tid: 1, ts: 0 },
        { ph: 'E', name: 'outer', pid: 1, tid: 1, ts: 150 },
      ],
    }),
    'utf8',
  );

  const summary = await summarizeTraceFile(trace, folded, svg, html);

  assert.equal(summary.event_count, 2);
  assert.match(await fs.readFile(folded, 'utf8'), /thread;outer 150/);
  assert.match(await fs.readFile(svg, 'utf8'), /<svg version="1.1"/);
  assert.match(await fs.readFile(html, 'utf8'), /Interactive flamegraph/);
  assert.match(await fs.readFile(html, 'utf8'), /addEventListener\('click'/);
});
