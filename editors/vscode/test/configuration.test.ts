import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

type PackageJson = {
  contributes?: {
    configuration?: {
      properties?: Record<string, unknown>;
    };
  };
};

function readConfigurationProperties(): Record<string, unknown> {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'),
  ) as PackageJson;

  return packageJson.contributes?.configuration?.properties ?? {};
}

test('contributes settings for the complete Vizsla user configuration surface', () => {
  const properties = readConfigurationProperties();
  const expectedSettings = [
    'vizsla.trace.server',
    'vizsla.server.command',
    'vizsla.server.args',
    'vizsla.server.cwd',
    'vizsla.server.additionalArgs',
    'vizsla.qihe.command',
    'vizsla.qihe.compileArgs',
    'vizsla.qihe.runArgs',
    'vizsla.files.excludeDirs',
    'vizsla.files.watcher',
    'vizsla.workspace.auto.reload',
    'vizsla.scope.visibility',
    'vizsla.formatter.provider',
    'vizsla.formatter.path',
    'vizsla.formatter.args',
    'vizsla.formatting.on.enter',
    'vizsla.formatting.in.comments',
    'vizsla.formatting.indent.width',
    'vizsla.inlayHints.port.connection.enable',
    'vizsla.inlayHints.parameter.assignment.enable',
    'vizsla.inlayHints.end.structure.enable',
    'vizsla.lens.instantiations.enable',
    'vizsla.semantic.tokens.port.clk.rst.enable',
    'vizsla.semantic.tokens.port.input.output.enable',
    'vizsla.diagnostics.enable',
    'vizsla.diagnostics.update',
    'vizsla.diagnostics.parse.enable',
    'vizsla.diagnostics.semantic.enable',
    'vizsla.diagnostics.slang.warnings',
    'vizsla.diagnostics.slang.rules',
    'vizsla.signature.help.params.only',
  ];

  assert.deepEqual(Object.keys(properties).sort(), expectedSettings.sort());
});

test('does not expose the old vizslaLsp settings namespace', () => {
  const properties = readConfigurationProperties();
  const oldSettings = Object.keys(properties).filter((key) => key.startsWith('vizslaLsp.'));

  assert.deepEqual(oldSettings, []);
});
