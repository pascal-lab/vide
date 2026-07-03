import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { describe, it } from 'node:test';

import type { PackageContext } from '../scripts/package/context';
import {
  restorePackageJson,
  stageDistFilesForTarget,
  stagePackageJsonForTarget,
} from '../scripts/package/manifest';
import { createPackagePlan } from '../scripts/package/targets';

describe('package content contract', () => {
  it('stages web packages without desktop entry points or node artifacts', () => {
    const context = temporaryPackageContext();
    writePackageJson(context, {
      main: './dist/extension.js',
      browser: './dist/browser/extension.js',
      contributes: {
        commands: [
          { command: 'vide.profileDiagnostics' },
          { command: 'vide.showOutput' },
        ],
      },
    });
    const nodeBundle = writeFile(
      context,
      'dist/extension.js',
      'require("vscode-languageclient/node"); require("./node/serverLaunch");',
    );
    const nodeArtifact = writeFile(
      context,
      'dist/node/serverLaunch.js',
      'require("node:fs");',
    );
    const browserBundle = writeFile(
      context,
      'dist/browser/extension.js',
      'require("vscode-languageclient/browser");',
    );

    const plan = createPackagePlan({
      target: 'web',
      profile: 'release',
      serverMode: 'build',
    });
    stageDistFilesForTarget(context, plan);
    const originalPackageJson = stagePackageJsonForTarget(context, plan);
    const packageJson = readPackageJson(context) as {
      main?: unknown;
      browser?: unknown;
      contributes?: { commands?: Array<{ command?: unknown }> };
    };

    assert.equal(packageJson.main, undefined);
    assert.equal(packageJson.browser, './dist/browser/extension.js');
    assert.equal(fs.existsSync(nodeBundle), false);
    assert.equal(fs.existsSync(nodeArtifact), false);
    assert.equal(fs.existsSync(browserBundle), true);
    assert.doesNotMatch(fs.readFileSync(browserBundle, 'utf8'), /vscode-languageclient\/node/);
    assert.doesNotMatch(fs.readFileSync(browserBundle, 'utf8'), /node\/serverLaunch/);
    assert.deepEqual(packageJson.contributes?.commands, [{ command: 'vide.showOutput' }]);

    restorePackageJson(context, originalPackageJson);
  });

  it('stages native packages without browser entry points or browser artifacts', () => {
    const context = temporaryPackageContext();
    writePackageJson(context, {
      main: './dist/extension.js',
      browser: './dist/browser/extension.js',
      contributes: {
        commands: [
          { command: 'vide.profileDiagnostics' },
          { command: 'vide.showOutput' },
        ],
      },
    });
    const nodeBundle = writeFile(context, 'dist/extension.js', 'desktop bundle');
    const browserBundle = writeFile(context, 'dist/browser/extension.js', 'browser bundle');

    const plan = createPackagePlan({
      target: 'linux-x64',
      profile: 'release',
      serverMode: 'build',
    });
    stageDistFilesForTarget(context, plan);
    const originalPackageJson = stagePackageJsonForTarget(context, plan);
    const packageJson = readPackageJson(context) as {
      main?: unknown;
      browser?: unknown;
      contributes?: { commands?: Array<{ command?: unknown }> };
    };

    assert.equal(packageJson.main, './dist/extension.js');
    assert.equal(packageJson.browser, undefined);
    assert.equal(fs.existsSync(nodeBundle), true);
    assert.equal(fs.existsSync(browserBundle), false);
    assert.deepEqual(packageJson.contributes?.commands, [{ command: 'vide.showOutput' }]);

    restorePackageJson(context, originalPackageJson);
  });
});

function temporaryPackageContext(): PackageContext {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vide-package-contract-'));
  return {
    vscodeDir: root,
    repoRoot: root,
  };
}

function writePackageJson(context: PackageContext, packageJson: unknown): void {
  fs.writeFileSync(
    path.join(context.vscodeDir, 'package.json'),
    `${JSON.stringify(packageJson, null, 2)}\n`,
  );
}

function readPackageJson(context: PackageContext): unknown {
  return JSON.parse(fs.readFileSync(path.join(context.vscodeDir, 'package.json'), 'utf8'));
}

function writeFile(context: PackageContext, relativePath: string, contents: string): string {
  const filePath = path.join(context.vscodeDir, relativePath);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, contents);
  return filePath;
}
