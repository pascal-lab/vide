import test from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  LEGACY_PROJECT_CONFIG_FILE_NAME,
  PROJECT_CONFIG_DOCUMENT_SELECTORS,
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_CONFIG_FILE_NAMES,
  PROJECT_SOURCE_FILE_GLOB_PATTERN,
  getProjectConfigPath,
  getProjectConfigPaths,
} from '../src/projectConfig';

test('uses the Vizsla project config file name', () => {
  assert.equal(PROJECT_CONFIG_FILE_NAME, 'vizsla.toml');
  assert.equal(LEGACY_PROJECT_CONFIG_FILE_NAME, 'vizsla_config.toml');
  assert.deepEqual(PROJECT_CONFIG_FILE_NAMES, ['vizsla.toml', 'vizsla_config.toml']);
});

test('selects project configs as LSP documents by file name', () => {
  assert.deepEqual(PROJECT_CONFIG_DOCUMENT_SELECTORS, [
    { scheme: 'file', pattern: '**/vizsla.toml' },
    { scheme: 'file', pattern: '**/vizsla_config.toml' },
  ]);
});

test('uses the VS Code language contribution source glob for startup config creation', () => {
  assert.equal(PROJECT_SOURCE_FILE_GLOB_PATTERN, '**/*.{v,vh,sv,svh,svi}');
});

test('resolves project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    getProjectConfigPath(workspaceRoot),
    path.join(workspaceRoot, PROJECT_CONFIG_FILE_NAME),
  );
});

test('resolves all supported project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.deepEqual(getProjectConfigPaths(workspaceRoot), [
    path.join(workspaceRoot, PROJECT_CONFIG_FILE_NAME),
    path.join(workspaceRoot, LEGACY_PROJECT_CONFIG_FILE_NAME),
  ]);
});

test('default project config keeps startup diagnostics syntax-only', () => {
  assert.equal(
    DEFAULT_PROJECT_CONFIG_TEXT,
    [
      '# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.',
      '# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.',
      'sources = []',
      'include_dirs = []',
      '',
    ].join('\n'),
  );
});
