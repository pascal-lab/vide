import * as fs from 'node:fs';
import * as path from 'node:path';

import type { PackageContext } from './context';
import { run } from './process';
import type { BuildProfile, NativeTargetSpec, ServerMode } from './targets';

const ALPINE_CARGO_TARGETS: Partial<Record<NativeTargetSpec['target'], string>> = {
  'alpine-arm64': 'aarch64-unknown-linux-musl',
  'alpine-x64': 'x86_64-unknown-linux-musl',
};

export function ensureTargetServerBinary(
  context: PackageContext,
  spec: NativeTargetSpec,
  profile: BuildProfile,
  serverMode: ServerMode,
  profileTrace: boolean,
): string {
  const args = [
    'xtask',
    'vscode',
    'prepare-server',
    '--target',
    spec.target,
    '--profile',
    profile,
    '--server',
    serverMode,
  ];
  if (profileTrace) {
    args.push('--profile-trace');
  }

  const serverPath = targetServerPath(context, spec);
  run('cargo', args, context.repoRoot, cargoXtaskEnvForTarget(spec));

  if (!fs.existsSync(serverPath)) {
    throw new Error(`prepared server binary was not found: ${serverPath}`);
  }

  return serverPath;
}

export function stageRuntimeServer(
  context: PackageContext,
  sourcePath: string,
  spec: NativeTargetSpec,
): string {
  const runtimeServerDir = path.join(context.vscodeDir, 'server');
  const runtimeServerPath = path.join(runtimeServerDir, spec.binaryFile);

  fs.mkdirSync(runtimeServerDir, { recursive: true });
  fs.copyFileSync(sourcePath, runtimeServerPath);
  if (!spec.isWindows) {
    fs.chmodSync(runtimeServerPath, 0o755);
  }

  return runtimeServerPath;
}

export function cleanRuntimeServerFiles(context: PackageContext): void {
  for (const binFile of ['vide.exe', 'vide']) {
    fs.rmSync(path.join(context.vscodeDir, 'server', binFile), { force: true });
  }
}

function targetServerPath(context: PackageContext, spec: NativeTargetSpec): string {
  return path.join(context.vscodeDir, 'server', spec.target, spec.binaryFile);
}

export function cargoXtaskEnvForTarget(
  spec: NativeTargetSpec,
  env: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
  const cargoTarget = ALPINE_CARGO_TARGETS[spec.target];
  if (!cargoTarget) {
    return env;
  }

  let updated = envWithoutCargoBuildTarget(env);
  const linkerEnvKey = cargoTargetLinkerEnvKey(cargoTarget);
  if (!optionalEnv(updated, linkerEnvKey)) {
    updated = {
      ...updated,
      [linkerEnvKey]: cargoLinkerForTarget(cargoTarget, updated),
    };
  }

  return appendRustFlags(updated, ['-C', 'link-arg=-lc']);
}

function envWithoutCargoBuildTarget(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const { CARGO_BUILD_TARGET: _cargoBuildTarget, ...updated } = env;
  return updated;
}

function cargoTargetLinkerEnvKey(cargoTarget: string): string {
  return `CARGO_TARGET_${cargoTargetEnvName(cargoTarget)}_LINKER`;
}

function cargoTargetEnvName(cargoTarget: string): string {
  return cargoTarget.toUpperCase().replace(/-/g, '_');
}

function cargoLinkerForTarget(cargoTarget: string, env: NodeJS.ProcessEnv): string {
  return (
    optionalEnv(env, cxxCompilerEnvKey(cargoTarget)) ??
    optionalEnv(env, 'TARGET_CXX') ??
    `${cargoTarget}-g++`
  );
}

function cxxCompilerEnvKey(cargoTarget: string): string {
  return `CXX_${cargoTarget.replace(/-/g, '_')}`;
}

function appendRustFlags(env: NodeJS.ProcessEnv, flags: string[]): NodeJS.ProcessEnv {
  const encodedFlags = optionalEnv(env, 'CARGO_ENCODED_RUSTFLAGS');
  if (encodedFlags) {
    return {
      ...env,
      CARGO_ENCODED_RUSTFLAGS: `${encodedFlags}\x1f${flags.join('\x1f')}`,
    };
  }

  const rustFlags = optionalEnv(env, 'RUSTFLAGS');
  return {
    ...env,
    RUSTFLAGS: rustFlags ? `${rustFlags} ${flags.join(' ')}` : flags.join(' '),
  };
}

function optionalEnv(env: NodeJS.ProcessEnv, name: string): string | undefined {
  const value = env[name]?.trim();
  return value ? value : undefined;
}
