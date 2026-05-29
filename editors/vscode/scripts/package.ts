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
const binName = 'vide';
const webTarget = 'web';

type BuildProfile = 'debug' | 'release';
type ServerMode = 'build' | 'prebuilt';
type PackageTarget = PlatformFolder | typeof webTarget;

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

function ensureServerExecutable(serverPath: string, target: PlatformFolder): void {
  if (!target.startsWith('win32-')) {
    fs.chmodSync(serverPath, 0o755);
  }
}

function ensureTargetServerBinary(
  target: PlatformFolder,
  binFile: string,
  profile: BuildProfile,
  serverMode: ServerMode,
): string {
  const serverOutDir = path.join(vscodeDir, 'server', target);
  const serverPath = path.join(serverOutDir, binFile);
  if (serverMode === 'prebuilt') {
    if (fs.existsSync(serverPath)) {
      ensureServerExecutable(serverPath, target);
      return serverPath;
    }
    throw new Error(`missing prebuilt server binary: ${serverPath}`);
  }

  const hostTarget = hostPlatformFolder();
  const cargoTarget = cargoTargets[target];
  if (target !== hostTarget && !cargoTarget) {
    throw new Error(
      `missing bundled server binary: ${serverPath}\n` +
        'tip: run packaging on a matching native runner or copy the target binary first.',
    );
  }

  if (cargoTarget) {
    run('rustup', ['target', 'add', cargoTarget], repoRoot);
  }

  run('cargo', cargoBuildArgs(profile, cargoTarget), repoRoot);

  const sourcePath = path.join(cargoOutputDir(profile, cargoTarget), binFile);
  const destPath = path.join(serverOutDir, binFile);
  fs.mkdirSync(serverOutDir, { recursive: true });
  fs.copyFileSync(sourcePath, destPath);
  ensureServerExecutable(destPath, target);

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

function syncReadmeFromRepoRoot(): void {
  fs.copyFileSync(path.join(repoRoot, 'README.md'), path.join(vscodeDir, 'README.md'));
}

function readExtensionVersion(): string {
  const packageJson = JSON.parse(fs.readFileSync(path.join(vscodeDir, 'package.json'), 'utf8')) as {
    version?: unknown;
  };
  if (typeof packageJson.version !== 'string' || packageJson.version.length === 0) {
    throw new Error('VS Code extension package.json must define a version.');
  }
  return packageJson.version;
}

function optionalEnv(name: string): string | undefined {
  const value = process.env[name]?.trim();
  return value ? value : undefined;
}

function writeBuildInfo(target: PackageTarget, profile: BuildProfile): void {
  const buildInfo = {
    version: readExtensionVersion(),
    target,
    profile,
    kind: optionalEnv('VIDE_EXTENSION_BUILD_KIND') ?? 'local',
    commitHash: optionalEnv('VIDE_EXTENSION_COMMIT_HASH'),
    buildDate: optionalEnv('VIDE_EXTENSION_BUILD_DATE'),
  };
  fs.writeFileSync(
    path.join(vscodeDir, 'build-info.json'),
    `${JSON.stringify(buildInfo, null, 2)}\n`,
  );
}

function parseServerMode(value: string): ServerMode {
  if (value === 'build' || value === 'prebuilt') {
    return value;
  }
  throw new Error(`unsupported server mode: ${value}`);
}

function parseArgs(): { target: PackageTarget; profile: BuildProfile; serverMode: ServerMode } {
  const args = process.argv.slice(2);
  let profile: BuildProfile = 'release';
  let serverMode: ServerMode = 'build';
  let target: string | undefined;

  for (const arg of args) {
    if (arg === '--debug') {
      profile = 'debug';
    } else if (arg.startsWith('--server=')) {
      serverMode = parseServerMode(arg.slice('--server='.length));
    } else if (!target) {
      target = arg;
    } else {
      throw new Error(`unexpected package argument: ${arg}`);
    }
  }

  target ??= hostPlatformFolder();
  if (target === webTarget) {
    return { target, profile, serverMode };
  }
  if (!isPlatformFolder(target)) {
    throw new Error(
      `unsupported target platform: ${target}\n` +
      `supported targets: ${[...SUPPORTED_PLATFORM_FOLDERS, webTarget].join(', ')}`,
    );
  }

  return { target, profile, serverMode };
}

function packageExtension(
  target: PackageTarget,
  profile: BuildProfile,
  serverMode: ServerMode,
): string {
  syncReadmeFromRepoRoot();
  writeBuildInfo(target, profile);

  const debugSuffix = profile === 'debug' ? '-debug' : '';
  const vsixOut = `vide-vscode-${target}${debugSuffix}.vsix`;
  const vsceBin = path.join(vscodeDir, 'node_modules', '@vscode', 'vsce', 'vsce');

  if (target === webTarget) {
    cleanRuntimeServerFiles();
    run(
      process.execPath,
      [vsceBin, 'package', '--target', target, '--out', vsixOut],
      vscodeDir,
      sanitizedVsceEnv(),
    );
    return path.join(vscodeDir, vsixOut);
  }

  const binFile = binaryFileForTarget(target);
  const targetServerPath = ensureTargetServerBinary(target, binFile, profile, serverMode);
  cleanRuntimeServerFiles();
  const runtimeServerPath = stageRuntimeServer(targetServerPath, target, binFile);

  try {
    run(
      process.execPath,
      [vsceBin, 'package', '--target', target, '--out', vsixOut],
      vscodeDir,
      sanitizedVsceEnv(),
    );
  } finally {
    fs.rmSync(runtimeServerPath, { force: true });
  }

  return path.join(vscodeDir, vsixOut);
}

function main(): void {
  const { target, profile, serverMode } = parseArgs();
  const vsixPath = packageExtension(target, profile, serverMode);
  console.log(vsixPath);
}

main();
