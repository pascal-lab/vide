import * as fs from 'node:fs';
import * as path from 'node:path';

import { type PackageContext, createPackageContext } from './context';
import {
  restorePackageJson,
  stagePackageJsonForTarget,
  syncReadmeFromRepoRoot,
  writeBuildInfo,
} from './manifest';
import { cleanRuntimeServerFiles, ensureTargetServerBinary, stageRuntimeServer } from './server';
import { type PackageOptions, createPackagePlan } from './targets';
import { runVscePackage } from './vsce';

export function packageExtension(
  options: PackageOptions,
  context: PackageContext = createPackageContext(),
): string {
  const plan = createPackagePlan(options);

  syncReadmeFromRepoRoot(context);
  writeBuildInfo(context, plan);

  if (plan.targetSpec.kind === 'web') {
    cleanRuntimeServerFiles(context);
    runVscePackage(context, plan);
    return path.join(context.vscodeDir, plan.vsixFile);
  }

  const targetServerPath = ensureTargetServerBinary(
    context,
    plan.targetSpec,
    plan.profile,
    plan.serverMode,
  );
  cleanRuntimeServerFiles(context);
  const runtimeServerPath = stageRuntimeServer(context, targetServerPath, plan.targetSpec);
  const originalPackageJson = stagePackageJsonForTarget(context, plan);

  try {
    runVscePackage(context, plan);
  } finally {
    fs.rmSync(runtimeServerPath, { force: true });
    restorePackageJson(context, originalPackageJson);
  }

  return path.join(context.vscodeDir, plan.vsixFile);
}
