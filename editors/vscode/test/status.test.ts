import test from 'node:test';
import assert from 'node:assert/strict';

import {
  asProjectStatus,
  defaultProjectStatusMessages,
  defaultServerStatusMessages,
  getProjectStatusPresentation,
  getServerStatusPresentation,
  getVideStatusPresentation,
  initialProjectStatus,
  selectVideStatusPhase,
  type ProjectStatus,
  type ServerStatus,
} from '../src/status';

test('maps server states to language status presentations', () => {
  const expected: Record<ServerStatus, { detail: string; busy: boolean; severity: string }> = {
    starting: {
      detail: 'Vide language server is starting.',
      busy: true,
      severity: 'information',
    },
    ready: {
      detail: 'Vide language server is running.',
      busy: false,
      severity: 'information',
    },
    stopping: {
      detail: 'Vide language server is stopping.',
      busy: true,
      severity: 'information',
    },
    stopped: {
      detail: 'Vide language server is stopped.',
      busy: false,
      severity: 'information',
    },
    error: {
      detail: 'Vide language server failed.',
      busy: false,
      severity: 'error',
    },
  };

  for (const [status, presentation] of Object.entries(expected)) {
    assert.deepEqual(getServerStatusPresentation(status as ServerStatus), {
      text: 'Vide',
      ...presentation,
    });
  }
});

test('includes detail in server status detail when available', () => {
  const presentation = getServerStatusPresentation('error', 'missing server binary');

  assert.equal(
    presentation.detail,
    'Vide language server failed.\nmissing server binary',
  );
  assert.equal(presentation.severity, 'error');
});

test('maps project status to language status presentations', () => {
  const baseStatus: ProjectStatus = {
    state: 'loaded',
    manifestUris: ['file:///workspace/vide.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
  };

  assert.deepEqual(getProjectStatusPresentation(baseStatus), {
    text: 'Vide',
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
    manifestUris: ['file:///workspace/vide.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
    message: 'workspace reload command',
  });

  assert.ok(status);
  assert.deepEqual(status, {
    state: 'loaded',
    manifestUris: ['file:///workspace/vide.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
    message: 'workspace reload command',
  });
  assert.equal(asProjectStatus({ ...status, state: 'unknown' }), undefined);
  assert.equal(asProjectStatus({ ...status, manifestUris: [1] }), undefined);
});

test('uses loading as the initial project status', () => {
  assert.deepEqual(initialProjectStatus(), {
    state: 'loading',
    manifestUris: [],
    unconfiguredRootUris: [],
    workspaceCount: 0,
    errors: [],
  });
});

test('selects the main Vide status from lifecycle order', () => {
  const projectStatus: ProjectStatus = {
    state: 'loaded',
    manifestUris: ['file:///workspace/vide.toml'],
    unconfiguredRootUris: [],
    workspaceCount: 1,
    errors: [],
  };

  assert.deepEqual(
    selectVideStatusPhase({
      serverStatus: 'starting',
      projectStatus,
    }),
    {
      kind: 'server',
      status: 'starting',
    },
  );
  assert.deepEqual(
    selectVideStatusPhase({
      serverStatus: 'ready',
      projectStatus,
    }),
    {
      kind: 'project',
      status: projectStatus,
    },
  );

  assert.equal(
    getVideStatusPresentation(
      {
        serverStatus: 'ready',
        projectStatus: { ...projectStatus, state: 'none', manifestUris: [] },
      },
      {
        server: defaultServerStatusMessages,
        project: defaultProjectStatusMessages,
      },
    ).detail,
    'No project manifest',
  );
});
