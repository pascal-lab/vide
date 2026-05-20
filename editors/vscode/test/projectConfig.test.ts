import test from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  LEGACY_PROJECT_CONFIG_FILE_NAME,
  PROJECT_CONFIG_FILE_NAME,
  isProjectConfigFileName,
  isProjectSourceFileName,
  getProjectConfigPath,
} from '../src/projectConfig';

test('uses the Vizsla project config file name', () => {
  assert.equal(PROJECT_CONFIG_FILE_NAME, 'vizsla.toml');
  assert.equal(LEGACY_PROJECT_CONFIG_FILE_NAME, 'vizsla_config.toml');
});

test('resolves project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    getProjectConfigPath(workspaceRoot),
    path.join(workspaceRoot, PROJECT_CONFIG_FILE_NAME),
  );
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
