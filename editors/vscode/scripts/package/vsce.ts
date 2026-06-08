import * as path from 'node:path';

import type { PackageContext } from './context';
import { run, sanitizedVsceEnv } from './process';
import type { PackagePlan } from './targets';

export function runVscePackage(context: PackageContext, plan: PackagePlan): void {
  const vsceBin = path.join(context.vscodeDir, 'node_modules', '@vscode', 'vsce', 'vsce');
  run(
    process.execPath,
    [vsceBin, 'package', '--target', plan.target, '--out', plan.vsixFile],
    context.vscodeDir,
    sanitizedVsceEnv(),
  );
}
