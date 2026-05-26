import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

interface VsixFile {
  path: string;
  mtimeMs: number;
}

function run(command: string, args: string[]): void {
  const isWin = process.platform === 'win32';
  const executable = isWin ? (process.env.ComSpec || 'cmd.exe') : command;
  const execArgs = isWin ? ['/d', '/s', '/c', command, ...args] : args;

  const result = spawnSync(executable, execArgs, { stdio: 'inherit' });

  if (result.error) {
    const errno = (result.error as NodeJS.ErrnoException).code;
    if (errno === 'ENOENT') {
      throw new Error(
        'Cannot find `code`. Install Visual Studio Code or add it to PATH and retry.',
      );
    }

    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
  }
}

function findVsixFiles(cwd: string): VsixFile[] {
  return fs
    .readdirSync(cwd)
    .filter((name) => /^vide-vscode-.*\.vsix$/i.test(name))
    .map((name) => {
      const absolute = path.join(cwd, name);
      const stat = fs.statSync(absolute);
      return { path: absolute, mtimeMs: stat.mtimeMs };
    });
}

function main(): void {
  const requested = process.argv[2];
  const cwd = process.cwd();
  const vsixFiles = findVsixFiles(cwd);

  if (vsixFiles.length === 0) {
    throw new Error(
      'No matching VSIX found. Run `npm run package:debug` first to create one, then rerun this command.',
    );
  }

  const candidates = requested
    ? vsixFiles.filter((file) => path.basename(file.path).includes(requested))
    : vsixFiles;

  if (candidates.length === 0) {
    throw new Error(
      `No VSIX matched the filter "${requested}". Available: ${vsixFiles.map((file) => file.path).join(', ')}`,
    );
  }

  if (candidates.length > 1 && !requested) {
    candidates.sort((a, b) => b.mtimeMs - a.mtimeMs);
    console.warn(
      `Multiple VSIX files found. Installing most recently modified: ${path.basename(
        candidates[0].path,
      )}`,
    );
  }

  const target = candidates[0].path;
  run('code', ['--install-extension', target]);
}

main();
