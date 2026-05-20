import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import {
  SUPPORTED_PLATFORM_FOLDERS,
  type PlatformFolder,
  getPlatformFolder,
  isPlatformFolder,
} from '../src/platform';

const vscodeDir = findExtensionRoot(__dirname);
const repoRoot = path.resolve(vscodeDir, '..', '..');
const binName = 'vizsla';

type BuildProfile = 'debug' | 'release';

const cargoTargets: Partial<Record<PlatformFolder, string>> = {
  'alpine-arm64': 'aarch64-unknown-linux-musl',
  'alpine-x64': 'x86_64-unknown-linux-musl',
};

function findExtensionRoot(startDir: string): string {
  let currentDir = path.resolve(startDir);

  while (true) {
    if (
      fs.existsSync(path.join(currentDir, 'package.json')) &&
      fs.existsSync(path.join(currentDir, 'language-configuration.json'))
    ) {
      return currentDir;
    }

    const parentDir = path.dirname(currentDir);
    if (parentDir === currentDir) {
      throw new Error(`could not find VS Code extension root from ${startDir}`);
    }
    currentDir = parentDir;
  }
}

function hostPlatformFolder(): PlatformFolder {
  const folder = getPlatformFolder(process.platform, process.arch);
  if (!folder) {
    throw new Error(`unsupported host platform: ${process.platform}-${process.arch}`);
  }

  return folder;
}

function binaryFileForTarget(target: PlatformFolder): string {
  return target.startsWith('win32-') ? `${binName}.exe` : binName;
}

function run(
  command: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv = process.env,
): void {
  const result = spawnSync(command, args, {
    cwd,
    env,
    shell: false,
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
  }
}

function sanitizedVsceEnv(): NodeJS.ProcessEnv {
  const env = { ...process.env };

  for (const key of Object.keys(env)) {
    const normalized = key.toLowerCase();
    if (
      normalized === 'npm_config_verify_deps_before_run' ||
      normalized === 'npm_config_npm_globalconfig' ||
      normalized === 'npm_config__jsr_registry'
    ) {
      delete env[key];
    }
  }

  return env;
}

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

function cargoOutputDir(profile: BuildProfile, cargoTarget?: string): string {
  const pathParts = [repoRoot, 'target'];
  if (cargoTarget) {
    pathParts.push(cargoTarget);
  }
  pathParts.push(cargoProfileDir(profile));

  return path.join(...pathParts);
}

function ensureTargetServerBinary(
  target: PlatformFolder,
  binFile: string,
  profile: BuildProfile,
): string {
  const serverOutDir = path.join(vscodeDir, 'server', target);
  const hostTarget = hostPlatformFolder();
  const cargoTarget = cargoTargets[target];
  if (target !== hostTarget && !cargoTarget) {
    const serverPath = path.join(serverOutDir, binFile);
    if (!fs.existsSync(serverPath)) {
      throw new Error(
        `missing bundled server binary: ${serverPath}\n` +
          'tip: run packaging on a matching native runner or copy the target binary first.',
      );
    }

    return serverPath;
  }

  if (cargoTarget) {
    run('rustup', ['target', 'add', cargoTarget], repoRoot);
  }

  run('cargo', cargoBuildArgs(profile, cargoTarget), repoRoot);

  const sourcePath = path.join(cargoOutputDir(profile, cargoTarget), binFile);
  const destPath = path.join(serverOutDir, binFile);
  fs.mkdirSync(serverOutDir, { recursive: true });
  fs.copyFileSync(sourcePath, destPath);
  if (!target.startsWith('win32-')) {
    fs.chmodSync(destPath, 0o755);
  }

  return destPath;
}

function stageRuntimeServer(sourcePath: string, target: PlatformFolder, binFile: string): string {
  const runtimeServerDir = path.join(vscodeDir, 'server');
  const runtimeServerPath = path.join(runtimeServerDir, binFile);

  fs.mkdirSync(runtimeServerDir, { recursive: true });
  fs.copyFileSync(sourcePath, runtimeServerPath);
  if (!target.startsWith('win32-')) {
    fs.chmodSync(runtimeServerPath, 0o755);
  }

  return runtimeServerPath;
}

function cleanRuntimeServerFiles(): void {
  for (const binFile of [`${binName}.exe`, binName]) {
    fs.rmSync(path.join(vscodeDir, 'server', binFile), { force: true });
  }
}

function parseArgs(): { target: PlatformFolder; profile: BuildProfile } {
  const args = process.argv.slice(2);
  const profile = args[0] === '--debug' ? 'debug' : 'release';
  if (profile === 'debug') {
    args.shift();
  }

  const target = args[0] ?? hostPlatformFolder();
  if (args.length > 1) {
    throw new Error(`unexpected package arguments: ${args.join(' ')}`);
  }

  if (!isPlatformFolder(target)) {
    throw new Error(
      `unsupported target platform: ${target}\n` +
      `supported targets: ${SUPPORTED_PLATFORM_FOLDERS.join(', ')}`,
    );
  }

  return { target, profile };
}

function packageExtension(target: PlatformFolder, profile: BuildProfile): string {
  const binFile = binaryFileForTarget(target);
  const targetServerPath = ensureTargetServerBinary(target, binFile, profile);
  cleanRuntimeServerFiles();
  const runtimeServerPath = stageRuntimeServer(targetServerPath, target, binFile);

  const debugSuffix = profile === 'debug' ? '-debug' : '';
  const vsixOut = `vizsla-vscode-${target}${debugSuffix}.vsix`;
  const vsceBin = path.join(vscodeDir, 'node_modules', '@vscode', 'vsce', 'vsce');
  try {
    run(process.execPath, [
      vsceBin,
      'package',
      '--target',
      target,
      '--out',
      vsixOut,
    ], vscodeDir, sanitizedVsceEnv());
  } finally {
    fs.rmSync(runtimeServerPath, { force: true });
  }

  return path.join(vscodeDir, vsixOut);
}

function main(): void {
  const { target, profile } = parseArgs();
  const vsixPath = packageExtension(target, profile);
  console.log(vsixPath);
}

main();
