import * as fs from 'node:fs';
import * as path from 'node:path';

const vscodeDir = path.resolve(__dirname, '..');

for (const dir of ['out', 'dist']) {
  fs.rmSync(path.join(vscodeDir, dir), { recursive: true, force: true });
}
