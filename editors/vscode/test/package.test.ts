import * as assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { parsePackageCliArgs } from '../scripts/package/cli';
import { createPackagePlan } from '../scripts/package/targets';

describe('package cli', () => {
  it('keeps the existing debug positional target syntax', () => {
    assert.deepEqual(parsePackageCliArgs(['--debug', 'linux-x64', '--server=prebuilt']), {
      target: 'linux-x64',
      profile: 'debug',
      serverMode: 'prebuilt',
    });
  });

  it('accepts explicit target and profile flags', () => {
    assert.deepEqual(parsePackageCliArgs(['--target', 'web', '--profile', 'release']), {
      target: 'web',
      profile: 'release',
      serverMode: 'build',
    });
  });
});

describe('package plan', () => {
  it('models web packages without native server staging', () => {
    const plan = createPackagePlan({
      target: 'web',
      profile: 'release',
      serverMode: 'build',
    });

    assert.equal(plan.target, 'web');
    assert.equal(plan.vsixFile, 'vide-vscode-web.vsix');
    assert.equal(plan.targetSpec.kind, 'web');
    assert.equal(plan.targetSpec.removeBrowserEntry, false);
  });

  it('models native debug packages with target binary metadata', () => {
    const plan = createPackagePlan({
      target: 'win32-x64',
      profile: 'debug',
      serverMode: 'prebuilt',
    });

    assert.equal(plan.target, 'win32-x64');
    assert.equal(plan.vsixFile, 'vide-vscode-win32-x64-debug.vsix');
    assert.equal(plan.targetSpec.kind, 'native');
    if (plan.targetSpec.kind === 'native') {
      assert.equal(plan.targetSpec.binaryFile, 'vide.exe');
      assert.equal(plan.targetSpec.isWindows, true);
      assert.equal(plan.targetSpec.removeBrowserEntry, true);
    }
  });
});
