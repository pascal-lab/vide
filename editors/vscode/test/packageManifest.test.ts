import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

type PackageJson = {
  activationEvents?: string[];
};

function readPackageJson(): PackageJson {
  return JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
}

test('activates when a workspace contains a Vizsla project config', () => {
  const packageJson = readPackageJson();

  assert.deepEqual(packageJson.activationEvents, [
    'onLanguage:verilog',
    'onLanguage:systemverilog',
    'workspaceContains:vizsla.toml',
    'workspaceContains:vizsla_config.toml',
  ]);
});
