import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const docsRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = resolve(docsRoot, 'dist', '404.html');
const target = resolve(docsRoot, 'dist', '404', 'index.html');

if (!existsSync(source)) {
  console.error(`Root 404 page not found: ${source}`);
  process.exit(1);
}

mkdirSync(dirname(target), { recursive: true });
copyFileSync(source, target);
console.log('Created /404/ alias for root 404 page.');
