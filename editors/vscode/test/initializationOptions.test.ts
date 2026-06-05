import test from 'node:test';
import assert from 'node:assert/strict';

import {
  diagnosticsProfilingInitializationOptions,
  serverInitializationOptions,
} from '../src/initializationOptions';
import { resolvedQiheCommand } from '../src/qiheCommand';

class TestConfiguration {
  constructor(private readonly values: Record<string, unknown>) {}

  get<T>(section: string): T | undefined {
    return this.values[section] as T | undefined;
  }
}

class TestVscodeConfiguration extends TestConfiguration {
  inspect<T>(section: string): { defaultValue?: T; globalValue?: T } | undefined {
    return {
      defaultValue: this.get<T>(section),
    };
  }
}

class TestConfiguredVscodeConfiguration extends TestConfiguration {
  inspect<T>(section: string): { defaultValue?: T; globalValue?: T } | undefined {
    return {
      defaultValue: (section === 'qihe.command' ? 'qihe' : undefined) as T | undefined,
      globalValue: this.get<T>(section),
    };
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

test('server initialization options treat the VS Code Qihe default as platform-owned', () => {
  const options = serverInitializationOptions(
    new TestVscodeConfiguration({
      'qihe.command': 'qihe',
    }),
    'win32',
  );

  assert.deepEqual(options.qihe, {
    command: 'qihe.bat',
    autoConfigureArgsFromManifest: true,
    compileArgs: [],
    runArgs: ['-g', 'std'],
  });
});

test('resolved Qihe command keeps explicit user configuration', () => {
  assert.equal(
    resolvedQiheCommand(new TestConfiguredVscodeConfiguration({ 'qihe.command': 'qihe' }), 'win32'),
    'qihe',
  );
  assert.equal(
    resolvedQiheCommand(
      new TestConfiguredVscodeConfiguration({ 'qihe.command': 'custom-qihe' }),
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
