import test from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';

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

function collectRuntimeL10nMessages(): string[] {
  const messages: string[] = [];
  for (const sourceFile of readSourceFiles(path.join(__dirname, '..', 'src'))) {
    const source = fs.readFileSync(sourceFile, 'utf8');
    const matches = source.matchAll(/vscode\.l10n\.t\(\s*'((?:\\'|[^'])*)'/g);
    messages.push(
      ...[...matches].map((match) => match[1].replace(/\\n/g, '\n').replace(/\\'/g, "'")),
    );
  }
  return [...new Set(messages)].sort();
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

test('localizes package contribution strings for English and Simplified Chinese', () => {
  const packageJson = readPackageJson();
  const placeholderKeys = [...collectNlsPlaceholders(packageJson)].sort();
  const englishKeys = Object.keys(readJson<Record<string, string>>('package.nls.json')).sort();
  const chineseKeys = Object.keys(readJson<Record<string, string>>('package.nls.zh-cn.json')).sort();

  assert.deepEqual(englishKeys, placeholderKeys);
  assert.deepEqual(chineseKeys, placeholderKeys);
});

test('localizes runtime extension strings for Simplified Chinese', () => {
  const packageJson = readPackageJson();
  assert.equal(packageJson.l10n, './l10n');

  const messages = collectRuntimeL10nMessages();
  const chineseBundle = readJson<Record<string, string>>(
    path.join('l10n', 'bundle.l10n.zh-cn.json'),
  );

  assert.deepEqual(Object.keys(chineseBundle).sort(), messages);
});
