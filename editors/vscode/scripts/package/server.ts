import * as fs from 'node:fs';
import * as path from 'node:path';

import type { PackageContext } from './context';
import { optionalEnv, run } from './process';
import {
  type BuildProfile,
  type NativeTargetSpec,
  type ServerMode,
  hostPlatformFolder,
} from './targets';

function cargoProfileDir(profile: BuildProfile): string {
  return profile === 'release' ? 'release' : 'debug';
}

function cargoBuildArgs(profile: BuildProfile, cargoTarget?: string): string[] {
  const args = ['build'];
  if (profile === 'release') {
    args.push('--release');
  }
  if (cargoTarget) {
    args.push('--target', cargoTarget);
  }

  return args;
}

function cargoTargetEnvName(cargoTarget: string): string {
  return cargoTarget.toUpperCase().replace(/-/g, '_');
}

function cargoTargetLinkerEnvKey(cargoTarget: string): string {
  return `CARGO_TARGET_${cargoTargetEnvName(cargoTarget)}_LINKER`;
}

function cxxCompilerEnvKey(cargoTarget: string): string {
  return `CXX_${cargoTarget.replace(/-/g, '_')}`;
}

function cargoLinkerForTarget(spec: NativeTargetSpec, cargoTarget: string): string | undefined {
  if (!spec.requiresAlpineLinker) {
    return undefined;
  }

  return (
    optionalEnv(cxxCompilerEnvKey(cargoTarget)) ??
    optionalEnv('TARGET_CXX') ??
    `${cargoTarget}-g++`
  );
}

function lateRustLinkFlagsForTarget(spec: NativeTargetSpec): string[] {
  if (!spec.requiresAlpineLinker) {
    return [];
  }

  // Static libstdc++ can introduce libc references after rustc's own musl -lc.
  return ['-C', 'link-arg=-lc'];
}

function appendRustFlags(env: NodeJS.ProcessEnv, flags: string[]): NodeJS.ProcessEnv {
  if (flags.length === 0) {
    return env;
  }

  const encodedFlags = env.CARGO_ENCODED_RUSTFLAGS;
  if (encodedFlags) {
    return {
      ...env,
      CARGO_ENCODED_RUSTFLAGS: `${encodedFlags}\x1f${flags.join('\x1f')}`,
    };
  }

  const rustFlags = env.RUSTFLAGS?.trim();
  return {
    ...env,
    RUSTFLAGS: rustFlags ? `${rustFlags} ${flags.join(' ')}` : flags.join(' '),
  };
}

function cargoBuildEnv(spec: NativeTargetSpec): NodeJS.ProcessEnv {
  if (!spec.cargoTarget) {
    return process.env;
  }

  let env = process.env;
  const linkerEnvKey = cargoTargetLinkerEnvKey(spec.cargoTarget);
  if (!optionalEnv(linkerEnvKey)) {
    const linker = cargoLinkerForTarget(spec, spec.cargoTarget);
    if (linker) {
      console.log(`Using Cargo linker for ${spec.cargoTarget}: ${linker}`);
      env = { ...env, [linkerEnvKey]: linker };
    }
  }

  const lateLinkArgs = lateRustLinkFlagsForTarget(spec);
  if (lateLinkArgs.length > 0) {
    console.log(`Adding Cargo link args for ${spec.cargoTarget}: ${lateLinkArgs.join(' ')}`);
    env = appendRustFlags(env, lateLinkArgs);
  }

  return env;
}

function cargoOutputDir(
  context: PackageContext,
  profile: BuildProfile,
  cargoTarget?: string,
): string {
  const pathParts = [context.repoRoot, 'target'];
  if (cargoTarget) {
    pathParts.push(cargoTarget);
  }
  pathParts.push(cargoProfileDir(profile));

  return path.join(...pathParts);
}

function ensureServerExecutable(serverPath: string, spec: NativeTargetSpec): void {
  if (!spec.isWindows) {
    fs.chmodSync(serverPath, 0o755);
  }
}

export function ensureTargetServerBinary(
  context: PackageContext,
  spec: NativeTargetSpec,
  profile: BuildProfile,
  serverMode: ServerMode,
): string {
  const serverOutDir = path.join(context.vscodeDir, 'server', spec.target);
  const serverPath = path.join(serverOutDir, spec.binaryFile);
  if (serverMode === 'prebuilt') {
    if (fs.existsSync(serverPath)) {
      ensureServerExecutable(serverPath, spec);
      return serverPath;
    }
    throw new Error(`missing prebuilt server binary: ${serverPath}`);
  }

  const hostTarget = hostPlatformFolder();
  if (spec.target !== hostTarget && !spec.cargoTarget) {
    throw new Error(
      `missing bundled server binary: ${serverPath}\n` +
        'tip: run packaging on a matching native runner or copy the target binary first.',
    );
  }

  if (spec.cargoTarget) {
    run('rustup', ['target', 'add', spec.cargoTarget], context.repoRoot);
  }

  run(
    'cargo',
    cargoBuildArgs(profile, spec.cargoTarget),
    context.repoRoot,
    cargoBuildEnv(spec),
  );

  const sourcePath = path.join(
    cargoOutputDir(context, profile, spec.cargoTarget),
    spec.binaryFile,
  );
  const destPath = path.join(serverOutDir, spec.binaryFile);
  fs.mkdirSync(serverOutDir, { recursive: true });
  fs.copyFileSync(sourcePath, destPath);
  ensureServerExecutable(destPath, spec);

  return destPath;
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
  ensureServerExecutable(runtimeServerPath, spec);

  return runtimeServerPath;
}

export function cleanRuntimeServerFiles(context: PackageContext): void {
  for (const binFile of ['vide.exe', 'vide']) {
    fs.rmSync(path.join(context.vscodeDir, 'server', binFile), { force: true });
  }
}
