import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  FUSE_SOC_CORE_FILE_GLOB,
  PROJECT_CONFIG_FILE_GLOB,
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
  PROJECT_CONFIG_FILE_NAME,
  isProjectConfigFileName,
  isProjectSourceFileName,
  getProjectConfigPath,
} from '../src/projectConfig';

test('uses the Vide project config file name', () => {
  assert.equal(PROJECT_CONFIG_FILE_NAME, 'vide.toml');
  assert.equal(PROJECT_CONFIG_FILE_GLOB, '**/vide.toml');
  assert.equal(FUSE_SOC_CORE_FILE_GLOB, '**/*.core');
});

test('resolves project config paths under workspace roots', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    getProjectConfigPath(workspaceRoot),
    path.join(workspaceRoot, PROJECT_CONFIG_FILE_NAME),
  );
});

test('recognizes project config file names', () => {
  assert.equal(isProjectConfigFileName('vide.toml'), true);
  assert.equal(isProjectConfigFileName('other.toml'), false);
});

test('activates the extension for FuseSoC projects', () => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'),
  ) as { activationEvents?: string[] };

  assert.ok(packageJson.activationEvents?.includes('workspaceContains:**/vide.toml'));
  assert.ok(packageJson.activationEvents?.includes('workspaceContains:**/*.core'));
});

test('recognizes Verilog and SystemVerilog source file names', () => {
  assert.equal(isProjectSourceFileName('top.v'), true);
  assert.equal(isProjectSourceFileName('top.SV'), true);
  assert.equal(isProjectSourceFileName('defs.svh'), true);
  assert.equal(isProjectSourceFileName('main.ts'), false);
});

test('default project config starts with empty analysis', () => {
  assert.equal(
    DEFAULT_PROJECT_CONFIG_TEXT,
    [
      `#:schema ${PROJECT_CONFIG_SCHEMA_URL}`,
      'sources = []',
      '',
      '# include_dirs = ["include"]',
      '# defines = ["SYNTHESIS"]',
      '# top_modules = ["top"]',
      '# libraries = ["../common_cells"]',
      '# exclude = ["build/**"]',
      '',
    ].join('\n'),
  );
});

test('project config schema URL resolves to a checked-in schema source', () => {
  const schemaUrl = new URL(PROJECT_CONFIG_SCHEMA_URL);

  assert.match(PROJECT_CONFIG_SCHEMA_VERSION, /^v\d+$/);
  assert.equal(schemaUrl.origin, 'https://vide.pascal-lab.net');
  assert.equal(schemaUrl.pathname, PROJECT_CONFIG_SCHEMA_PATH);
  assert.equal(
    PROJECT_CONFIG_SCHEMA_PATH,
    `/schemas/${PROJECT_CONFIG_SCHEMA_VERSION}/vide.schema.json`,
  );
  assert.match(schemaUrl.pathname, /^\/schemas\/v\d+\/vide\.schema\.json$/);

  const repoRoot = path.join(__dirname, '..', '..', '..');
  const schemaPath = path.join(repoRoot, schemaUrl.pathname.replace(/^\//, ''));
  assert.equal(fs.existsSync(schemaPath), true);

  const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8')) as { $id?: string };
  assert.equal(schema.$id, PROJECT_CONFIG_SCHEMA_URL);
});
