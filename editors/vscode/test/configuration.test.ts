import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

import { getL10nJson, type IScriptFile } from '@vscode/l10n-dev';

type PackageJson = {
  l10n?: string;
  contributes?: {
    configuration?: {
      properties?: Record<string, unknown>;
    };
  };
};

function readJson<T>(fileName: string): T {
  return JSON.parse(fs.readFileSync(path.join(__dirname, '..', fileName), 'utf8')) as T;
}

function readPackageJson(): PackageJson {
  return readJson<PackageJson>('package.json');
}

function readConfigurationProperties(): Record<string, unknown> {
  const packageJson = readPackageJson();

  return packageJson.contributes?.configuration?.properties ?? {};
}

function collectNlsPlaceholders(value: unknown, keys = new Set<string>()): Set<string> {
  if (typeof value === 'string') {
    const match = /^%([^%]+)%$/.exec(value);
    if (match) {
      keys.add(match[1]);
    }
    return keys;
  }

  if (Array.isArray(value)) {
    for (const item of value) {
      collectNlsPlaceholders(item, keys);
    }
    return keys;
  }

  if (value && typeof value === 'object') {
    for (const item of Object.values(value)) {
      collectNlsPlaceholders(item, keys);
    }
  }

  return keys;
}

async function collectRuntimeL10nMessages(): Promise<string[]> {
  const sourceFiles = readSourceFiles(path.join(__dirname, '..', 'src'));
  const scriptFiles: IScriptFile[] = sourceFiles.map((sourceFile) => ({
    contents: fs.readFileSync(sourceFile, 'utf8'),
    extension: path.extname(sourceFile),
  }));
  const messages = await getL10nJson(scriptFiles);

  return Object.keys(messages).sort();
}

function readSourceFiles(dir: string): string[] {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      return readSourceFiles(entryPath);
    }
    return entry.isFile() && entry.name.endsWith('.ts') ? [entryPath] : [];
  });
}

test('contributes settings for the complete Vide user configuration surface', () => {
  const properties = readConfigurationProperties();
  const expectedSettings = [
    'vide.trace.server',
    'vide.server.command',
    'vide.server.args',
    'vide.server.cwd',
    'vide.server.additionalArgs',
    'vide.qihe.command',
    'vide.qihe.autoConfigureArgsFromManifest',
    'vide.qihe.compileArgs',
    'vide.qihe.runArgs',
    'vide.files.excludeDirs',
    'vide.files.watcher',
    'vide.workspace.auto.reload',
    'vide.scope.visibility',
    'vide.references.includeDeclaration',
    'vide.formatter.provider',
    'vide.formatter.path',
    'vide.formatter.args',
    'vide.formatting.on.enter',
    'vide.formatting.in.comments',
    'vide.formatting.indent.width',
    'vide.inlayHints.port.connection.enable',
    'vide.inlayHints.parameter.assignment.enable',
    'vide.inlayHints.end.structure.enable',
    'vide.lens.instantiations.enable',
    'vide.semantic.tokens.port.clk.rst.enable',
    'vide.semantic.tokens.port.input.output.enable',
    'vide.diagnostics.enable',
    'vide.diagnostics.update',
    'vide.diagnostics.parse.enable',
    'vide.diagnostics.semantic.enable',
    'vide.diagnostics.slang.warnings',
    'vide.diagnostics.slang.rules',
    'vide.signature.help.params.only',
  ];

  assert.deepEqual(Object.keys(properties).sort(), expectedSettings.sort());
});

test('does not expose the old videLsp settings namespace', () => {
  const properties = readConfigurationProperties();
  const oldSettings = Object.keys(properties).filter((key) => key.startsWith('videLsp.'));

  assert.deepEqual(oldSettings, []);
});

test('localizes package contribution strings for English and Simplified Chinese', () => {
  const packageJson = readPackageJson();
  const placeholderKeys = [...collectNlsPlaceholders(packageJson)].sort();
  const englishKeys = Object.keys(readJson<Record<string, string>>('package.nls.json')).sort();
  const chineseKeys = Object.keys(readJson<Record<string, string>>('package.nls.zh-cn.json')).sort();

  assert.deepEqual(englishKeys, placeholderKeys);
  assert.deepEqual(chineseKeys, placeholderKeys);
});

test('localizes runtime extension strings for Simplified Chinese', async () => {
  const packageJson = readPackageJson();
  assert.equal(packageJson.l10n, './l10n');

  const messages = await collectRuntimeL10nMessages();
  const chineseBundle = readJson<Record<string, string>>(
    path.join('l10n', 'bundle.l10n.zh-cn.json'),
  );

  assert.deepEqual(Object.keys(chineseBundle).sort(), messages);
});

test('diagnostic action labels use diagnostic scope wording', async () => {
  const messages = await collectRuntimeL10nMessages();

  assert.equal(messages.some((message) => message.includes('diagnostic {0}')), false);
  assert.ok(messages.includes('Ignore this diagnostic type in workspace settings'));
  assert.ok(messages.includes('Downgrade this diagnostic type to warning in workspace settings'));
  assert.ok(messages.includes('Ignore this diagnostic type in user settings'));
  assert.ok(messages.includes('Downgrade this diagnostic type to warning in user settings'));
});
