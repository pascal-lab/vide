#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const rootArg = args.find((arg) => arg !== '--dry-run');
const root = rootArg ?? process.cwd();
const lockPath = join(root, 'package-lock.json');
const lock = JSON.parse(readFileSync(lockPath, 'utf8'));
const packages = lock.packages ?? {};
const libc = detectLibc();
const specs = new Map();

for (const [ownerKey, owner] of Object.entries(packages)) {
  const optionalDependencies = owner.optionalDependencies ?? {};
  for (const [name, declaredVersion] of Object.entries(optionalDependencies)) {
    const packageKey = dependencyPackageKey(ownerKey, name);
    const packageMeta = packages[packageKey] ?? packages[`node_modules/${name}`];
    if (!isCurrentPlatformOptional(name, packageMeta)) {
      continue;
    }
    if (existsSync(join(root, packageKey))) {
      continue;
    }

    const version = packageMeta?.version ?? declaredVersion;
    if (!isExactVersion(version)) {
      throw new Error(
        `Cannot install ${name} from non-exact optional dependency version ${version}`,
      );
    }
    specs.set(name, `${name}@${version}`);
  }
}

if (specs.size === 0) {
  console.log('Native optional dependencies are already present.');
  process.exit(0);
}

const installArgs = ['install', '--no-save', '--package-lock=false', ...specs.values()];
console.log(`Installing native optional dependencies: ${[...specs.values()].join(' ')}`);
if (dryRun) {
  process.exit(0);
}

const result = spawnSync('npm', installArgs, { cwd: root, stdio: 'inherit' });
process.exit(result.status ?? 1);

function dependencyPackageKey(ownerKey, dependencyName) {
  if (!ownerKey || !ownerKey.includes('/node_modules/')) {
    return `node_modules/${dependencyName}`;
  }
  return `${ownerKey.slice(0, ownerKey.lastIndexOf('/node_modules/'))}/node_modules/${dependencyName}`;
}

function isCurrentPlatformOptional(name, packageMeta) {
  if (packageMeta && !allows(packageMeta.os, process.platform)) {
    return false;
  }
  if (packageMeta && !allows(packageMeta.cpu, process.arch)) {
    return false;
  }
  if (packageMeta && !allows(packageMeta.libc, libc)) {
    return false;
  }
  if (packageMeta) {
    return true;
  }

  const normalizedName = name.replace('/', '-');
  if (process.platform === 'linux') {
    if (!normalizedName.includes('linux')) {
      return false;
    }
    if (!normalizedName.includes(process.arch)) {
      return false;
    }
    if (normalizedName.includes('musl')) {
      return libc === 'musl';
    }
    if (normalizedName.includes('gnu')) {
      return libc === 'glibc';
    }
    return true;
  }

  if (process.platform === 'darwin') {
    return normalizedName.includes('darwin') && normalizedName.includes(process.arch);
  }

  if (process.platform === 'win32') {
    return normalizedName.includes('win32') && normalizedName.includes(process.arch);
  }

  return normalizedName.includes(process.platform) && normalizedName.includes(process.arch);
}

function allows(values, current) {
  if (!values || values.length === 0) {
    return true;
  }
  const blocked = values.some((value) => value === `!${current}`);
  if (blocked) {
    return false;
  }
  const allowed = values.filter((value) => !value.startsWith('!'));
  return allowed.length === 0 || allowed.includes(current);
}

function isExactVersion(version) {
  return /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version);
}

function detectLibc() {
  if (process.platform !== 'linux') {
    return undefined;
  }
  return process.report?.getReport?.().header?.glibcVersionRuntime ? 'glibc' : 'musl';
}
