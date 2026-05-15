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

function buildServer(target: PlatformFolder, serverOutDir: string, binFile: string): void {
  const hostTarget = hostPlatformFolder();
  if (target !== hostTarget) {
    const serverPath = path.join(serverOutDir, binFile);
    if (!fs.existsSync(serverPath)) {
      throw new Error(
        `missing bundled server binary: ${serverPath}\n` +
          'tip: run packaging on a matching native runner or copy the target binary first.',
      );
    }

    return;
  }

  run('cargo', ['build', '--release'], repoRoot);

  const sourcePath = path.join(repoRoot, 'target', 'release', binFile);
  const destPath = path.join(serverOutDir, binFile);
  fs.copyFileSync(sourcePath, destPath);
  if (!target.startsWith('win32-')) {
    fs.chmodSync(destPath, 0o755);
  }
}

function packageExtension(target: string): string {
  if (!isPlatformFolder(target)) {
    throw new Error(
      `unsupported target platform: ${target}\n` +
        `supported targets: ${SUPPORTED_PLATFORM_FOLDERS.join(', ')}`,
    );
  }

  const serverOutDir = path.join(vscodeDir, 'server', target);
  fs.mkdirSync(serverOutDir, { recursive: true });

  const binFile = binaryFileForTarget(target);
  buildServer(target, serverOutDir, binFile);

  const vsixOut = `vizsla-vscode-${target}.vsix`;
  const vsceBin = path.join(vscodeDir, 'node_modules', '@vscode', 'vsce', 'vsce');
  run(process.execPath, [
    vsceBin,
    'package',
    '--target',
    target,
    '--ignore-other-target-folders',
    '--out',
    vsixOut,
  ], vscodeDir, sanitizedVsceEnv());

  return path.join(vscodeDir, vsixOut);
}

function main(): void {
  const target = process.argv[2] ?? hostPlatformFolder();
  const vsixPath = packageExtension(target);
  console.log(vsixPath);
}

main();
