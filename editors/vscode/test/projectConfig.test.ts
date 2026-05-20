import test from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_FILE_NAME,
  getProjectConfigPath,
} from '../src/projectConfig';

test('uses the Vizsla project config file name', () => {
  assert.equal(PROJECT_CONFIG_FILE_NAME, 'vizsla_config.toml');
});

test('resolves project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    getProjectConfigPath(workspaceRoot),
    path.join(workspaceRoot, PROJECT_CONFIG_FILE_NAME),
  );
});

test('default project config keeps startup diagnostics syntax-only', () => {
  assert.equal(
    DEFAULT_PROJECT_CONFIG_TEXT,
    [
      '# Syntax-only startup config. Keep these empty arrays to avoid scanning the workspace.',
      '# Do not delete them unless you want omitted fields to default to the workspace root.',
      '# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.',
      'sources = []',
      'include_dirs = []',
      '',
    ].join('\n'),
  );
});
