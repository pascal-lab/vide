import * as fs from 'node:fs';
import * as path from 'node:path';

const vscodeDir = path.resolve(__dirname, '..');
const speedscopePackageJson = require.resolve('speedscope/package.json') as string;
const speedscopeReleaseDir = path.join(path.dirname(speedscopePackageJson), 'dist', 'release');
const outputDir = path.join(vscodeDir, 'dist', 'speedscope');

if (!fs.existsSync(path.join(speedscopeReleaseDir, 'index.html'))) {
  throw new Error(`Speedscope release assets not found at ${speedscopeReleaseDir}`);
}

fs.rmSync(outputDir, { recursive: true, force: true });
fs.cpSync(speedscopeReleaseDir, outputDir, { recursive: true });
