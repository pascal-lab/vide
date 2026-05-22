import test from 'node:test';
import assert from 'node:assert/strict';

import {
  diagnosticCodeSelector,
  diagnosticOptionName,
  diagnosticSelectorLabel,
  upsertDiagnosticRule,
} from '../src/diagnosticRules';

test('builds code selectors from slang diagnostic codes', () => {
  assert.equal(diagnosticCodeSelector({ source: 'slang', code: '6:129' }), 'code:6:129');
});

test('builds code selectors from VS Code diagnostic code objects', () => {
  assert.equal(
    diagnosticCodeSelector({ source: 'slang', code: { value: '2:260' } }),
    'code:2:260',
  );
});

test('ignores diagnostics that cannot be configured by slang code', () => {
  assert.equal(diagnosticCodeSelector({ source: 'vizsla', code: '2:260' }), undefined);
  assert.equal(diagnosticCodeSelector({ source: 'slang', code: 260 }), undefined);
  assert.equal(diagnosticCodeSelector({ source: 'slang', code: 'bad' }), undefined);
});

test('renders concise diagnostic selector labels', () => {
  assert.equal(diagnosticSelectorLabel('code:6:129'), 'this diagnostic type');
});

test('reads slang diagnostic option names for source-local actions', () => {
  assert.equal(
    diagnosticOptionName({ source: 'slang', data: { option: 'unconnected-port' } }),
    'unconnected-port',
  );
  assert.equal(diagnosticOptionName({ source: 'vizsla', data: { option: 'foo' } }), undefined);
  assert.equal(diagnosticOptionName({ source: 'slang', data: { option: null } }), undefined);
});

test('upserts diagnostic severity rules by selector', () => {
  assert.deepEqual(
    upsertDiagnosticRule(
      [{ selector: 'code:6:129', severity: 'error', force: true }],
      'code:6:129',
      'warning',
    ),
    [{ selector: 'code:6:129', severity: 'warning', force: true }],
  );

  assert.deepEqual(upsertDiagnosticRule([], 'code:2:260', 'ignore'), [
    { selector: 'code:2:260', severity: 'ignore' },
  ]);
});
