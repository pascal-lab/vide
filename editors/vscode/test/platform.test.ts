import test from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';

import {
  SUPPORTED_PLATFORM_FOLDERS,
  getBundledServerPath,
  getPlatformFolder,
  getServerBinaryName,
  isPlatformFolder,
} from '../src/platform';

test('maps supported Node platform and architecture pairs to VS Code target folders', () => {
  assert.equal(getPlatformFolder('alpine', 'arm64'), 'alpine-arm64');
  assert.equal(getPlatformFolder('alpine', 'x64'), 'alpine-x64');
  assert.equal(getPlatformFolder('darwin', 'arm64'), 'darwin-arm64');
  assert.equal(getPlatformFolder('darwin', 'x64'), 'darwin-x64');
  assert.equal(getPlatformFolder('linux', 'arm64'), 'linux-arm64');
  assert.equal(getPlatformFolder('linux', 'x64'), 'linux-x64');
  assert.equal(getPlatformFolder('win32', 'arm64'), 'win32-arm64');
  assert.equal(getPlatformFolder('win32', 'x64'), 'win32-x64');
});

test('rejects unsupported platform and architecture pairs', () => {
  assert.equal(getPlatformFolder('freebsd', 'x64'), undefined);
  assert.equal(getPlatformFolder('linux', 'ia32'), undefined);
});

test('checks package targets with a type guard', () => {
  assert.equal(isPlatformFolder('alpine-arm64'), true);
  assert.equal(isPlatformFolder('alpine-x64'), true);
  assert.equal(isPlatformFolder('linux-x64'), true);
  assert.equal(isPlatformFolder('linux-riscv64'), false);
});

test('uses Windows executable names only for Windows targets', () => {
  assert.equal(getServerBinaryName('win32'), 'vide.exe');
  assert.equal(getServerBinaryName('linux'), 'vide');
  assert.equal(getServerBinaryName('darwin'), 'vide');
});

test('resolves bundled server paths for every packaged target', () => {
  const extensionPath = path.join('tmp', 'vide-extension');

  for (const target of SUPPORTED_PLATFORM_FOLDERS) {
    const [platform, arch] = target.split('-');
    const binaryName = platform === 'win32' ? 'vide.exe' : 'vide';
    assert.equal(
      getBundledServerPath(extensionPath, platform, arch),
      path.join(extensionPath, 'server', binaryName),
    );
  }
});
