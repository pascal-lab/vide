import test from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  LEGACY_PROJECT_CONFIG_FILE_NAME,
  PROJECT_CONFIG_DOCUMENT_SELECTORS,
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_CONFIG_FILE_NAMES,
  PROJECT_SOURCE_FILE_GLOB,
  isProjectConfigFileName,
  isProjectSourceFileName,
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
  assert.equal(PROJECT_SOURCE_FILE_GLOB, '**/*.{v,sv,vh,svh,svi}');
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

test('resolves legacy project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    getProjectConfigPath(workspaceRoot, LEGACY_PROJECT_CONFIG_FILE_NAME),
    path.join(workspaceRoot, LEGACY_PROJECT_CONFIG_FILE_NAME),
  );
});

test('recognizes project config file names', () => {
  assert.equal(isProjectConfigFileName('vizsla.toml'), true);
  assert.equal(isProjectConfigFileName('vizsla_config.toml'), true);
  assert.equal(isProjectConfigFileName('other.toml'), false);
});

test('recognizes Verilog and SystemVerilog source file names', () => {
  assert.equal(isProjectSourceFileName('top.v'), true);
  assert.equal(isProjectSourceFileName('top.SV'), true);
  assert.equal(isProjectSourceFileName('defs.svh'), true);
  assert.equal(isProjectSourceFileName('main.ts'), false);
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
