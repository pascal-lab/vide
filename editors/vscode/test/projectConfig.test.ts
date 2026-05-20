import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
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
      `#:schema ${PROJECT_CONFIG_SCHEMA_URL}`,
      '# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.',
      '# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.',
      'sources = []',
      'include_dirs = []',
      '',
    ].join('\n'),
  );
});

test('project config schema URL resolves to a published docs asset', () => {
  const schemaUrl = new URL(PROJECT_CONFIG_SCHEMA_URL);

  assert.match(PROJECT_CONFIG_SCHEMA_VERSION, /^v\d+$/);
  assert.equal(schemaUrl.origin, 'https://pascal-lab.github.io');
  assert.equal(schemaUrl.pathname, PROJECT_CONFIG_SCHEMA_PATH);
  assert.equal(
    PROJECT_CONFIG_SCHEMA_PATH,
    `/vizsla/schemas/${PROJECT_CONFIG_SCHEMA_VERSION}/vizsla.schema.json`,
  );
  assert.match(schemaUrl.pathname, /^\/vizsla\/schemas\/v\d+\/vizsla\.schema\.json$/);

  const docsPublicPath = path.join(__dirname, '..', '..', '..', 'docs', 'public');
  const schemaPath = path.join(docsPublicPath, schemaUrl.pathname.replace(/^\/vizsla\//, ''));
  assert.equal(fs.existsSync(schemaPath), true);

  const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8')) as { $id?: string };
  assert.equal(schema.$id, PROJECT_CONFIG_SCHEMA_URL);
});
