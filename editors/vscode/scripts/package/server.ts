import * as fs from 'node:fs';
import * as path from 'node:path';

import type { PackageContext } from './context';
import { run } from './process';
import type { BuildProfile, NativeTargetSpec, ServerMode } from './targets';

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
  run('cargo', args, context.repoRoot);

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


