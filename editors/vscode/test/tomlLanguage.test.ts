import test from 'node:test';
import assert from 'node:assert/strict';

import { TOML_LANGUAGE_CONFIGURATION, TOML_LANGUAGE_ID } from '../src/tomlLanguage';

test('registers TOML bracket and quote pairs for project configs', () => {
  assert.equal(TOML_LANGUAGE_ID, 'toml');
  assert.deepEqual(TOML_LANGUAGE_CONFIGURATION.comments, { lineComment: '#' });
  assert.deepEqual(TOML_LANGUAGE_CONFIGURATION.brackets, [['[', ']']]);
  assert.deepEqual(TOML_LANGUAGE_CONFIGURATION.autoClosingPairs, [
    { open: '[', close: ']' },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ]);
});
