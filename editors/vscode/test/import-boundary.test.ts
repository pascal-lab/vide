import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { describe, it } from 'node:test';

const repoDir = path.resolve(__dirname, '..');
const srcDir = path.join(repoDir, 'src');

const nodeImportPattern =
  /\b(?:from\s+['"](?:node:|fs(?:\/promises)?|path|child_process|vscode-languageclient\/node)|import\s*\(\s*['"](?:node:|fs(?:\/promises)?|path|child_process|vscode-languageclient\/node))/;
const browserOnlyClientPattern =
  /\b(?:from\s+['"]vscode-languageclient\/browser|import\s*\(\s*['"]vscode-languageclient\/browser)/;
const nodeServerLaunchPattern =
  /\b(?:from\s+['"][^'"]*node\/serverLaunch|import\s*\(\s*['"][^'"]*node\/serverLaunch)/;

describe('extension import boundaries', () => {
  it('keeps common code runtime-neutral', () => {
    assertBoundary('common', (filePath, text) => {
      assertNoMatch(filePath, text, nodeImportPattern, 'common code must not import Node APIs');
      assertNoMatch(
        filePath,
        text,
        browserOnlyClientPattern,
        'common code must not import browser-only language client APIs',
      );
      assertNoMatch(
        filePath,
        text,
        nodeServerLaunchPattern,
        'common code must not import node/serverLaunch',
      );
    });
  });

  it('keeps browser code away from Node-only modules', () => {
    assertBoundary('browser', (filePath, text) => {
      assertNoMatch(filePath, text, nodeImportPattern, 'browser code must not import Node APIs');
      assertNoMatch(
        filePath,
        text,
        nodeServerLaunchPattern,
        'browser code must not import node/serverLaunch',
      );
    });
  });
});

function assertBoundary(
  subdir: 'browser' | 'common',
  assertion: (filePath: string, text: string) => void,
): void {
  const dir = path.join(srcDir, subdir);
  if (!fs.existsSync(dir)) {
    return;
  }

  for (const filePath of sourceFiles(dir)) {
    assertion(filePath, fs.readFileSync(filePath, 'utf8'));
  }
}

function sourceFiles(dir: string): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];

  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...sourceFiles(entryPath));
    } else if (entry.isFile() && entry.name.endsWith('.ts')) {
      files.push(entryPath);
    }
  }

  return files;
}

function assertNoMatch(
  filePath: string,
  text: string,
  pattern: RegExp,
  message: string,
): void {
  assert.equal(
    pattern.test(text),
    false,
    `${message}: ${path.relative(repoDir, filePath)}`,
  );
}
