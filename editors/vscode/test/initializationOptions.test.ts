import test from 'node:test';
import assert from 'node:assert/strict';

import {
  diagnosticsProfilingInitializationOptions,
  serverInitializationOptions,
} from '../src/common/initializationOptions';
import { USER_CONFIG_SETTINGS } from '../src/generated/configuration';
import { resolvedQiheCommand } from '../src/qiheCommand';

class TestConfiguration {
  constructor(private readonly values: Record<string, unknown>) {}

  get<T>(section: string): T | undefined {
    return this.values[section] as T | undefined;
  }
}

test('server initialization options include user configuration for startup', () => {
  const options = serverInitializationOptions(
    new TestConfiguration({
      'files.excludeDirs': ['build'],
      'files.watcher': 'notify',
      'diagnostics.semantic.enable': false,
      'diagnostics.slang.rules': [{ selector: 'source:parse', severity: 'ignore' }],
      'qihe.command': 'custom-qihe',
    }),
  );

  assert.deepEqual(options.files, {
    excludeDirs: ['build'],
    watcher: 'notify',
  });
  assert.deepEqual(options.diagnostics, {
    enable: true,
    update: 'onSave',
    parse: { enable: true },
    semantic: { enable: false },
    slang: {
      warnings: ['width-expand', 'width-trunc', 'port-width-expand', 'port-width-trunc'],
      rules: [{ selector: 'source:parse', severity: 'ignore' }],
    },
  });
  assert.deepEqual(options.qihe, {
    command: 'custom-qihe',
    autoConfigureArgsFromManifest: true,
    compileArgs: [],
    runArgs: ['-g', 'std'],
  });
});

test('server initialization options pass the Qihe platform default marker to the server', () => {
  const options = serverInitializationOptions(
    new TestConfiguration({
      'qihe.command': null,
    }),
  );

  assert.deepEqual(options.qihe, {
    command: null,
    autoConfigureArgsFromManifest: true,
    compileArgs: [],
    runArgs: ['-g', 'std'],
  });
});

test('generated Qihe command setting uses the platform default marker', () => {
  const setting = USER_CONFIG_SETTINGS.find(
    (configSetting) => configSetting.vscodeSection === 'qihe.command',
  );

  assert.equal(setting?.defaultValue, null);
});

test('resolved Qihe command uses the platform default for unset extension commands', () => {
  assert.equal(resolvedQiheCommand(new TestConfiguration({}), 'win32'), 'qihe.bat');
  assert.equal(resolvedQiheCommand(new TestConfiguration({ 'qihe.command': null }), 'win32'), 'qihe.bat');
  assert.equal(resolvedQiheCommand(new TestConfiguration({}), 'linux'), 'qihe');
});

test('resolved Qihe command keeps explicit user configuration', () => {
  assert.equal(
    resolvedQiheCommand(new TestConfiguration({ 'qihe.command': 'qihe' }), 'win32'),
    'qihe',
  );
  assert.equal(
    resolvedQiheCommand(
      new TestConfiguration({ 'qihe.command': 'custom-qihe' }),
      'win32',
    ),
    'custom-qihe',
  );
});

test('diagnostics profiling initialization options reuse startup options with server watching', () => {
  const options = diagnosticsProfilingInitializationOptions(
    new TestConfiguration({
      'files.excludeDirs': ['build'],
      'files.watcher': 'client',
      'diagnostics.semantic.enable': false,
    }),
  );

  assert.deepEqual(options.files, {
    excludeDirs: ['build'],
    watcher: 'server',
  });
  assert.deepEqual(options.diagnostics, {
    enable: true,
    update: 'onSave',
    parse: { enable: true },
    semantic: { enable: false },
    slang: {
      warnings: ['width-expand', 'width-trunc', 'port-width-expand', 'port-width-trunc'],
      rules: [],
    },
  });
});
