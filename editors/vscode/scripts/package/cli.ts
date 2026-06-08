import {
  type BuildProfile,
  type PackageOptions,
  type ServerMode,
  parseBuildProfile,
  parseServerMode,
} from './targets';

export function parsePackageCliArgs(args: string[]): PackageOptions {
  let profile: BuildProfile = 'release';
  let serverMode: ServerMode = 'build';
  let profileTrace = false;
  let target: string | undefined;

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === '--debug') {
      profile = 'debug';
    } else if (arg === '--release') {
      profile = 'release';
    } else if (arg === '--profile-trace') {
      profileTrace = true;
    } else if (arg === '--target') {
      target = readFlagValue(args, ++index, arg);
    } else if (arg.startsWith('--target=')) {
      target = arg.slice('--target='.length);
    } else if (arg === '--profile') {
      profile = parseBuildProfile(readFlagValue(args, ++index, arg));
    } else if (arg.startsWith('--profile=')) {
      profile = parseBuildProfile(arg.slice('--profile='.length));
    } else if (arg === '--server') {
      serverMode = parseServerMode(readFlagValue(args, ++index, arg));
    } else if (arg.startsWith('--server=')) {
      serverMode = parseServerMode(arg.slice('--server='.length));
    } else if (!arg.startsWith('-') && !target) {
      target = arg;
    } else {
      throw new Error(`unexpected package argument: ${arg}`);
    }
  }

  return { target, profile, serverMode, profileTrace };
}

function readFlagValue(args: string[], index: number, flag: string): string {
  const value = args[index];
  if (!value || value.startsWith('-')) {
    throw new Error(`missing value for ${flag}`);
  }
  return value;
}
