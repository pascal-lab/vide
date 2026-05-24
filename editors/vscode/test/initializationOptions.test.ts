import test from 'node:test';
import assert from 'node:assert/strict';

import {
  diagnosticsProfilingInitializationOptions,
  serverInitializationOptions,
} from '../src/initializationOptions';

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
      warnings: [],
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
      warnings: [],
      rules: [],
    },
  });
});
