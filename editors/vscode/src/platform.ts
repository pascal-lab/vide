import * as path from 'node:path';

export const SUPPORTED_PLATFORM_FOLDERS = [
  'alpine-arm64',
  'alpine-x64',
  'darwin-arm64',
  'darwin-x64',
  'linux-arm64',
  'linux-x64',
  'win32-arm64',
  'win32-x64',
] as const;

export type PlatformFolder = (typeof SUPPORTED_PLATFORM_FOLDERS)[number];

const supportedPlatformFolders = new Set<string>(SUPPORTED_PLATFORM_FOLDERS);

export function isPlatformFolder(value: string): value is PlatformFolder {
  return supportedPlatformFolders.has(value);
}

export function getPlatformFolder(
  platform: string = process.platform,
  arch: string = process.arch,
): PlatformFolder | undefined {
  const platformFolder = `${platform}-${arch}`;
  return isPlatformFolder(platformFolder) ? platformFolder : undefined;
}

export function getServerBinaryName(platform: string = process.platform): string {
  return platform === 'win32' ? 'vizsla.exe' : 'vizsla';
}

export function getBundledServerPath(
  extensionPath: string,
  platform: string = process.platform,
  arch: string = process.arch,
): string | undefined {
  const platformFolder = getPlatformFolder(platform, arch);
  if (!platformFolder) {
    return undefined;
  }

  return path.join(extensionPath, 'server', getServerBinaryName(platform));
}
