import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createVideCodeDocumentSelector,
  createVideDocumentSelector,
  fileCodeDocumentSelector,
  fileDocumentSelector,
} from '../src/common/documentSelector';

test('creates file-scoped selectors for native extension clients', () => {
  assert.deepEqual(fileDocumentSelector, [
    { scheme: 'file', language: 'verilog' },
    { scheme: 'file', language: 'systemverilog' },
  ]);
  assert.deepEqual(fileCodeDocumentSelector, fileDocumentSelector);
});

test('creates scheme-neutral selectors for browser extension clients', () => {
  assert.deepEqual(createVideDocumentSelector(), [
    { language: 'verilog' },
    { language: 'systemverilog' },
  ]);
  assert.deepEqual(createVideCodeDocumentSelector(), createVideDocumentSelector());
});
