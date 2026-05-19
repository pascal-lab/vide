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

test('default project config explicitly scans the workspace root', () => {
  assert.match(DEFAULT_PROJECT_CONFIG_TEXT, /^sources = \["\."\]$/m);
  assert.match(DEFAULT_PROJECT_CONFIG_TEXT, /^include_dirs = \["\."\]$/m);
  assert.equal(DEFAULT_PROJECT_CONFIG_TEXT.endsWith('\n'), true);
});
