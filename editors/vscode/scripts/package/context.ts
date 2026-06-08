import * as fs from 'node:fs';
import * as path from 'node:path';

export interface PackageContext {
  vscodeDir: string;
  repoRoot: string;
}

export function createPackageContext(startDir: string = __dirname): PackageContext {
  const vscodeDir = findExtensionRoot(startDir);
  return {
    vscodeDir,
    repoRoot: path.resolve(vscodeDir, '..', '..'),
  };
}

export function findExtensionRoot(startDir: string): string {
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
