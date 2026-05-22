import test from 'node:test';
import assert from 'node:assert/strict';

import {
  asProjectStatus,
  getProjectStatusPresentation,
  getServerStatusPresentation,
  projectStatusFallback,
  type ProjectStatus,
  type ServerStatus,
} from '../src/status';

test('maps server states to concise status bar labels', () => {
  const expected: Record<ServerStatus, string> = {
    starting: '$(loading~spin) Vizsla',
    ready: 'Vizsla',
    stopping: '$(loading~spin) Vizsla',
    stopped: '$(circle-slash) Vizsla',
    error: '$(error) Vizsla',
  };

  for (const [status, text] of Object.entries(expected)) {
    assert.equal(getServerStatusPresentation(status as ServerStatus).text, text);
  }
});

test('includes detail in status tooltip when available', () => {
  const presentation = getServerStatusPresentation('error', 'missing server binary');

  assert.equal(
    presentation.tooltip,
    'Vizsla language server failed.\nmissing server binary',
  );
  assert.equal(presentation.backgroundColor, 'statusBarItem.errorBackground');
});

test('maps project status to language status presentations', () => {
  const baseStatus: ProjectStatus = {
    state: 'loaded',
    manifestUris: ['file:///workspace/vizsla.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
  };

  assert.deepEqual(getProjectStatusPresentation(baseStatus), {
    text: 'Vizsla',
    detail: 'Project manifest loaded',
    severity: 'information',
    busy: false,
  });

  assert.equal(
    getProjectStatusPresentation({ ...baseStatus, state: 'loading' }).busy,
    true,
  );
  assert.equal(
    getProjectStatusPresentation({ ...baseStatus, state: 'none', manifestUris: [] }).severity,
    'warning',
  );
  assert.equal(
    getProjectStatusPresentation({ ...baseStatus, state: 'error', errors: ['bad toml'] }).severity,
    'error',
  );
});

test('parses project status notifications defensively', () => {
  const status = asProjectStatus({
    state: 'loaded',
    manifestUris: ['file:///workspace/vizsla.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
    message: 'workspace reload command',
  });

  assert.ok(status);
  assert.deepEqual(status, {
    state: 'loaded',
    manifestUris: ['file:///workspace/vizsla.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
    message: 'workspace reload command',
  });
  assert.equal(asProjectStatus({ ...status, state: 'unknown' }), undefined);
  assert.equal(asProjectStatus({ ...status, manifestUris: [1] }), undefined);
});

test('uses loading as the project status fallback', () => {
  assert.deepEqual(projectStatusFallback(), {
    state: 'loading',
    manifestUris: [],
    unconfiguredRootUris: [],
    workspaceCount: 0,
    errors: [],
  });
});
