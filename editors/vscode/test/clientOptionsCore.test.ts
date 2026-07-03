import test from 'node:test';
import assert from 'node:assert/strict';
import type { LanguageClientOptions } from 'vscode-languageclient';

import { createVideClientOptionsCore } from '../src/common/clientOptionsCore';
import { createVideDocumentSelector } from '../src/common/documentSelector';

type ClientMiddleware = NonNullable<LanguageClientOptions['middleware']>;

class TestConfiguration {
  constructor(private readonly values: Record<string, unknown>) {}

  get<T>(section: string): T | undefined {
    return this.values[section] as T | undefined;
  }
}

test('creates the runtime-neutral client options core', () => {
  const documentSelector = createVideDocumentSelector();
  const provideRenameEdits: NonNullable<ClientMiddleware['provideRenameEdits']> =
    async () => undefined;

  const options = createVideClientOptionsCore({
    documentSelector,
    configuration: new TestConfiguration({
      'files.excludeDirs': ['build'],
      'diagnostics.semantic.enable': false,
    }),
    provideRenameEdits,
  });

  assert.equal(options.documentSelector, documentSelector);
  assert.equal(options.middleware.provideRenameEdits, provideRenameEdits);
  assert.deepEqual(options.initializationOptions.files, {
    excludeDirs: ['build'],
    watcher: 'client',
  });
  assert.deepEqual(options.initializationOptions.diagnostics, {
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
