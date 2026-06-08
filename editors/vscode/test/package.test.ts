import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { describe, it } from 'node:test';

import type { PackageContext } from '../scripts/package/context';
import { parsePackageCliArgs } from '../scripts/package/cli';
import {
  restorePackageJson,
  stagePackageJsonForTarget,
  stageProfileTraceAssets,
} from '../scripts/package/manifest';
import { cargoXtaskEnvForTarget } from '../scripts/package/server';
import { createPackagePlan, type NativeTargetSpec } from '../scripts/package/targets';

describe('package cli', () => {
  it('keeps the existing debug positional target syntax', () => {
    assert.deepEqual(parsePackageCliArgs(['--debug', 'linux-x64', '--server=prebuilt']), {
      target: 'linux-x64',
      profile: 'debug',
      serverMode: 'prebuilt',
      profileTrace: false,
    });
  });

  it('accepts explicit target and profile flags', () => {
    assert.deepEqual(
      parsePackageCliArgs(['--target', 'web', '--profile', 'release', '--profile-trace']),
      {
        target: 'web',
        profile: 'release',
        serverMode: 'build',
        profileTrace: true,
      },
    );
  });

  it('leaves profile trace disabled by default', () => {
    assert.deepEqual(parsePackageCliArgs(['--target', 'web', '--profile', 'release']), {
      target: 'web',
      profile: 'release',
      serverMode: 'build',
      profileTrace: false,
    });
  });
});

describe('package staging', () => {
  it('removes profiling command contributions when profile trace is disabled', () => {
    const context = temporaryPackageContext();
    fs.writeFileSync(
      path.join(context.vscodeDir, 'package.json'),
      `${JSON.stringify(
        {
          browser: './dist/browser/extension.js',
          contributes: {
            commands: [
              { command: 'vide.profileDiagnostics' },
              { command: 'vide.showOutput' },
            ],
          },
        },
        null,
        2,
      )}\n`,
    );

    const plan = createPackagePlan({
      target: 'linux-x64',
      profile: 'release',
      serverMode: 'build',
    });
    const originalPackageJson = stagePackageJsonForTarget(context, plan);
    const packageJson = JSON.parse(
      fs.readFileSync(path.join(context.vscodeDir, 'package.json'), 'utf8'),
    ) as {
      browser?: unknown;
      contributes?: { commands?: Array<{ command?: unknown }> };
    };

    assert.equal(packageJson.browser, undefined);
    assert.deepEqual(packageJson.contributes?.commands, [{ command: 'vide.showOutput' }]);

    restorePackageJson(context, originalPackageJson);
    assert.match(
      fs.readFileSync(path.join(context.vscodeDir, 'package.json'), 'utf8'),
      /vide\.profileDiagnostics/,
    );
  });

  it('removes stale profile trace assets when profile trace is disabled', () => {
    const context = temporaryPackageContext();
    const speedscopeDir = path.join(context.vscodeDir, 'dist', 'speedscope');
    fs.mkdirSync(speedscopeDir, { recursive: true });
    fs.writeFileSync(path.join(speedscopeDir, 'index.html'), '');

    const plan = createPackagePlan({
      target: 'web',
      profile: 'release',
      serverMode: 'build',
    });
    stageProfileTraceAssets(context, plan);

    assert.equal(fs.existsSync(speedscopeDir), false);
  });
});

describe('package server preparation', () => {
  it('removes inherited Cargo build targets before cargo xtask is built', () => {
    const baseEnv = { CARGO_BUILD_TARGET: 'x86_64-unknown-linux-musl' };
    const env = cargoXtaskEnvForTarget(nativeTargetSpec('alpine-x64'), baseEnv);

    assert.equal(env.CARGO_BUILD_TARGET, undefined);
    assert.equal(
      env.CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER,
      'x86_64-unknown-linux-musl-g++',
    );
    assert.equal(env.RUSTFLAGS, '-C link-arg=-lc');
    assert.equal(baseEnv.CARGO_BUILD_TARGET, 'x86_64-unknown-linux-musl');
  });

  it('appends Alpine cargo flags to encoded rust flags', () => {
    const env = cargoXtaskEnvForTarget(nativeTargetSpec('alpine-arm64'), {
      CARGO_ENCODED_RUSTFLAGS: '-C\x1ftarget-feature=+crt-static',
      CXX_aarch64_unknown_linux_musl: 'custom-aarch64-g++',
    });

    assert.equal(env.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER, 'custom-aarch64-g++');
    assert.equal(
      env.CARGO_ENCODED_RUSTFLAGS,
      '-C\x1ftarget-feature=+crt-static\x1f-C\x1flink-arg=-lc',
    );
  });

  it('leaves non-Alpine cargo env untouched', () => {
    const baseEnv = { RUSTFLAGS: '-Dwarnings' };

    assert.equal(
      cargoXtaskEnvForTarget(nativeTargetSpec('win32-x64', 'vide.exe'), baseEnv),
      baseEnv,
    );
  });
});

function temporaryPackageContext(): PackageContext {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vide-package-'));
  return {
    vscodeDir: root,
    repoRoot: root,
  };
}

function nativeTargetSpec(
  target: NativeTargetSpec['target'],
  binaryFile = 'vide',
): NativeTargetSpec {
  return {
    kind: 'native',
    target,
    binaryFile,
    isWindows: target.startsWith('win32-'),
    removeBrowserEntry: true,
  };
}

describe('package plan', () => {
  it('models web packages without native server staging', () => {
    const plan = createPackagePlan({
      target: 'web',
      profile: 'release',
      serverMode: 'build',
    });

    assert.equal(plan.target, 'web');
    assert.equal(plan.profileTrace, false);
    assert.equal(plan.vsixFile, 'vide-vscode-web.vsix');
    assert.equal(plan.targetSpec.kind, 'web');
    assert.equal(plan.targetSpec.removeBrowserEntry, false);
  });

  it('models native debug packages with target binary metadata', () => {
    const plan = createPackagePlan({
      target: 'win32-x64',
      profile: 'debug',
      serverMode: 'prebuilt',
      profileTrace: true,
    });

    assert.equal(plan.target, 'win32-x64');
    assert.equal(plan.profileTrace, true);
    assert.equal(plan.vsixFile, 'vide-vscode-win32-x64-debug.vsix');
    assert.equal(plan.targetSpec.kind, 'native');
    if (plan.targetSpec.kind === 'native') {
      assert.equal(plan.targetSpec.binaryFile, 'vide.exe');
      assert.equal(plan.targetSpec.isWindows, true);
      assert.equal(plan.targetSpec.removeBrowserEntry, true);
    }
  });
});
