import test from 'node:test';
import assert from 'node:assert/strict';

import { USER_CONFIG_SETTINGS } from '../src/generated/configuration';
import { affectsBrowserClientConfiguration } from '../src/browser/configuration';

function changedSection(section: string): { affectsConfiguration(candidate: string): boolean } {
  return {
    affectsConfiguration: (candidate) => candidate === section,
  };
}

test('browser restarts for generated user configuration settings', () => {
  for (const setting of USER_CONFIG_SETTINGS) {
    assert.equal(
      affectsBrowserClientConfiguration(changedSection(setting.vscodeKey)),
      true,
      setting.vscodeKey,
    );
  }
});

test('browser ignores desktop-only launch configuration settings', () => {
  for (const section of [
    'vide.server.command',
    'vide.server.args',
    'vide.server.additionalArgs',
    'vide.server.cwd',
    'vide.trace.server',
  ]) {
    assert.equal(affectsBrowserClientConfiguration(changedSection(section)), false, section);
  }
});
