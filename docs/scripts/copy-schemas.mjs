import { cp, rm } from 'node:fs/promises';
import { resolve } from 'node:path';

const docsRoot = resolve(import.meta.dirname, '..');
const repoRoot = resolve(docsRoot, '..');
const source = resolve(repoRoot, 'schemas');
const target = resolve(docsRoot, 'public', 'schemas');

await rm(target, { recursive: true, force: true });
await cp(source, target, { recursive: true });
