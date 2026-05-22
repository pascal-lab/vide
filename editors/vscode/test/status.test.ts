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

test('maps server states to language status presentations', () => {
  const expected: Record<ServerStatus, { detail: string; busy: boolean; severity: string }> = {
    starting: {
      detail: 'Vizsla language server is starting.',
      busy: true,
      severity: 'information',
    },
    ready: {
      detail: 'Vizsla language server is running.',
      busy: false,
      severity: 'information',
    },
    stopping: {
      detail: 'Vizsla language server is stopping.',
      busy: true,
      severity: 'information',
    },
    stopped: {
      detail: 'Vizsla language server is stopped.',
      busy: false,
      severity: 'information',
    },
    error: {
      detail: 'Vizsla language server failed.',
      busy: false,
      severity: 'error',
    },
  };

  for (const [status, presentation] of Object.entries(expected)) {
    assert.deepEqual(getServerStatusPresentation(status as ServerStatus), {
      text: 'Vizsla',
      ...presentation,
    });
  }
});

test('includes detail in server status detail when available', () => {
  const presentation = getServerStatusPresentation('error', 'missing server binary');

  assert.equal(
    presentation.detail,
    'Vizsla language server failed.\nmissing server binary',
  );
  assert.equal(presentation.severity, 'error');
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
