import {
  SUPPORTED_PLATFORM_FOLDERS,
  type PlatformFolder,
  getPlatformFolder,
  isPlatformFolder,
} from '../../src/platform';

export const WEB_TARGET = 'web';

export type BuildProfile = 'debug' | 'release';
export type ServerMode = 'build' | 'prebuilt';
export type PackageTarget = PlatformFolder | typeof WEB_TARGET;

export interface PackageOptions {
  target?: string;
  profile: BuildProfile;
  serverMode: ServerMode;
}

export interface WebTargetSpec {
  kind: 'web';
  target: typeof WEB_TARGET;
  removeBrowserEntry: false;
}

export interface NativeTargetSpec {
  kind: 'native';
  target: PlatformFolder;
  binaryFile: string;
  isWindows: boolean;
  removeBrowserEntry: true;
}

export type TargetSpec = WebTargetSpec | NativeTargetSpec;

export interface PackagePlan {
  target: PackageTarget;
  profile: BuildProfile;
  serverMode: ServerMode;
  targetSpec: TargetSpec;
  vsixFile: string;
}

export function createPackagePlan(options: PackageOptions): PackagePlan {
  const target = resolvePackageTarget(options.target);
  const targetSpec = targetSpecFor(target);
  const debugSuffix = options.profile === 'debug' ? '-debug' : '';

  return {
    target,
    profile: options.profile,
    serverMode: options.serverMode,
    targetSpec,
    vsixFile: `vide-vscode-${target}${debugSuffix}.vsix`,
  };
}

export function parseBuildProfile(value: string): BuildProfile {
  if (value === 'debug' || value === 'release') {
    return value;
  }
  throw new Error(`unsupported build profile: ${value}`);
}

export function parseServerMode(value: string): ServerMode {
  if (value === 'build' || value === 'prebuilt') {
    return value;
  }
  throw new Error(`unsupported server mode: ${value}`);
}

export function hostPlatformFolder(): PlatformFolder {
  const folder = getPlatformFolder(process.platform, process.arch);
  if (!folder) {
    throw new Error(`unsupported host platform: ${process.platform}-${process.arch}`);
  }

  return folder;
}

function resolvePackageTarget(target: string | undefined): PackageTarget {
  target ??= hostPlatformFolder();
  if (target === WEB_TARGET) {
    return target;
  }
  if (isPlatformFolder(target)) {
    return target;
  }

  throw new Error(
    `unsupported target platform: ${target}\n` +
      `supported targets: ${[...SUPPORTED_PLATFORM_FOLDERS, WEB_TARGET].join(', ')}`,
  );
}

function targetSpecFor(target: PackageTarget): TargetSpec {
  if (target === WEB_TARGET) {
    return {
      kind: 'web',
      target,
      removeBrowserEntry: false,
    };
  }

  return {
    kind: 'native',
    target,
    binaryFile: binaryFileForTarget(target),
    isWindows: target.startsWith('win32-'),
    removeBrowserEntry: true,
  };
}

function binaryFileForTarget(target: PlatformFolder): string {
  return target.startsWith('win32-') ? 'vide.exe' : 'vide';
}
