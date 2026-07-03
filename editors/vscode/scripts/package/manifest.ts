import * as fs from 'node:fs';
import * as path from 'node:path';

import type { PackageContext } from './context';
import { optionalEnv } from './process';
import type { PackagePlan } from './targets';

export function syncReadmeFromRepoRoot(context: PackageContext): void {
  fs.copyFileSync(
    path.join(context.repoRoot, 'README.md'),
    path.join(context.vscodeDir, 'README.md'),
  );
}

export function writeBuildInfo(context: PackageContext, plan: PackagePlan): void {
  const buildInfo = {
    version: readExtensionVersion(context),
    target: plan.target,
    profile: plan.profile,
    profileTrace: plan.profileTrace,
    kind: optionalEnv('VIDE_EXTENSION_BUILD_KIND') ?? 'local',
    commitHash: optionalEnv('VIDE_EXTENSION_COMMIT_HASH'),
    buildDate: optionalEnv('VIDE_EXTENSION_BUILD_DATE'),
  };
  fs.writeFileSync(
    path.join(context.vscodeDir, 'build-info.json'),
    `${JSON.stringify(buildInfo, null, 2)}\n`,
  );
}

export function stagePackageJsonForTarget(
  context: PackageContext,
  plan: PackagePlan,
): string | undefined {
  const packagePath = packageJsonPath(context);
  const originalPackageJson = fs.readFileSync(packagePath, 'utf8');
  const packageJson = JSON.parse(originalPackageJson) as {
    main?: unknown;
    browser?: unknown;
    contributes?: { commands?: Array<{ command?: unknown }> };
  };
  if (plan.targetSpec.kind === 'web') {
    delete packageJson.main;
  } else if (plan.targetSpec.removeBrowserEntry) {
    delete packageJson.browser;
  }
  if (!plan.profileTrace) {
    packageJson.contributes = packageJson.contributes ?? {};
    packageJson.contributes.commands = (packageJson.contributes.commands ?? []).filter(
      (command) => command.command !== 'vide.profileDiagnostics',
    );
  }
  fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
  return originalPackageJson;
}

export function stageDistFilesForTarget(context: PackageContext, plan: PackagePlan): void {
  const distDir = path.join(context.vscodeDir, 'dist');
  if (plan.targetSpec.kind === 'web') {
    fs.rmSync(path.join(distDir, 'extension.js'), { force: true });
    return;
  }

  fs.rmSync(path.join(distDir, 'browser'), { recursive: true, force: true });
}

export function stageProfileTraceAssets(context: PackageContext, plan: PackagePlan): void {
  const speedscopeDir = path.join(context.vscodeDir, 'dist', 'speedscope');
  if (!plan.profileTrace) {
    fs.rmSync(speedscopeDir, { recursive: true, force: true });
    return;
  }

  const indexPath = path.join(speedscopeDir, 'index.html');
  if (!fs.existsSync(indexPath)) {
    throw new Error(
      `profile trace assets not found at ${speedscopeDir}; run npm run compile:profile-trace first`,
    );
  }
}

export function restorePackageJson(
  context: PackageContext,
  originalPackageJson: string | undefined,
): void {
  if (originalPackageJson) {
    fs.writeFileSync(packageJsonPath(context), originalPackageJson);
  }
}

function packageJsonPath(context: PackageContext): string {
  return path.join(context.vscodeDir, 'package.json');
}

function readExtensionVersion(context: PackageContext): string {
  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath(context), 'utf8')) as {
    version?: unknown;
  };
  if (typeof packageJson.version !== 'string' || packageJson.version.length === 0) {
    throw new Error('VS Code extension package.json must define a version.');
  }
  return packageJson.version;
}
