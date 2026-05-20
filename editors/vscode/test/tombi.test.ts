import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import {
  TOMBI_SCHEMA_URL,
  ensureTombiSchemaConfigFile,
  ensureTombiSchemaConfigText,
  userTombiConfigPath,
  workspaceTombiConfigPath,
} from '../src/tombi';

test('resolves Tombi user config paths by platform', () => {
  assert.equal(
    userTombiConfigPath('win32', { APPDATA: 'C:\\Users\\me\\AppData\\Roaming' }, 'C:\\Users\\me'),
    'C:\\Users\\me\\AppData\\Roaming\\tombi\\config.toml',
  );
  assert.equal(
    userTombiConfigPath('darwin', {}, '/Users/me'),
    '/Users/me/Library/Application Support/tombi/config.toml',
  );
  assert.equal(
    userTombiConfigPath('linux', { XDG_CONFIG_HOME: '/home/me/.config2' }, '/home/me'),
    '/home/me/.config2/tombi/config.toml',
  );
  assert.equal(
    userTombiConfigPath('linux', {}, '/home/me'),
    '/home/me/.config/tombi/config.toml',
  );
});

test('prefers an existing dot Tombi config in workspace scope', () => {
  const workspaceRoot = path.join('tmp', 'workspace');
  const dotConfig = path.join(workspaceRoot, '.tombi.toml');
  const exists = (filePath: string) => filePath === dotConfig;

  assert.equal(workspaceTombiConfigPath(workspaceRoot, exists), dotConfig);
});

test('uses tombi.toml when no workspace Tombi config exists', () => {
  const workspaceRoot = path.join('tmp', 'workspace');

  assert.equal(
    workspaceTombiConfigPath(workspaceRoot, () => false),
    path.join(workspaceRoot, 'tombi.toml'),
  );
});

test('injects the Vizsla schema block into Tombi config text', () => {
  const result = ensureTombiSchemaConfigText('[formatter]\n');

  assert.equal(result.changed, true);
  assert.match(result.text, /\[\[schemas\]\]/);
  assert.match(result.text, new RegExp(TOMBI_SCHEMA_URL.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  assert.match(result.text, /include = \["\*\*\/vizsla\.toml", "\*\*\/vizsla_config\.toml"\]/);
});

test('does not inject duplicate Vizsla schema blocks', () => {
  const text = `[[schemas]]\npath = "${TOMBI_SCHEMA_URL}"\n`;
  const result = ensureTombiSchemaConfigText(text);

  assert.equal(result.changed, false);
  assert.equal(result.text, text);
});

test('creates Tombi config files when injecting schema config', async () => {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'vizsla-tombi-'));
  const configPath = path.join(dir, 'tombi', 'config.toml');

  const result = await ensureTombiSchemaConfigFile(configPath);
  const text = await fs.promises.readFile(configPath, 'utf8');

  assert.equal(result, 'created');
  assert.match(text, /\[\[schemas\]\]/);
});

test('keeps existing Tombi schema config files idempotent', async () => {
  const dir = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'vizsla-tombi-'));
  const configPath = path.join(dir, 'config.toml');
  await fs.promises.writeFile(configPath, `[[schemas]]\npath = "${TOMBI_SCHEMA_URL}"\n`, 'utf8');

  const result = await ensureTombiSchemaConfigFile(configPath);

  assert.equal(result, 'already-configured');
});
