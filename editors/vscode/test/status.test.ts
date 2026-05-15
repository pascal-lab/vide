import test from 'node:test';
import assert from 'node:assert/strict';

import { getServerStatusPresentation, type ServerStatus } from '../src/status';

test('maps server states to concise status bar labels', () => {
  const expected: Record<ServerStatus, string> = {
    starting: '$(sync~spin) Vizsla Starting',
    ready: '$(check) Vizsla Ready',
    stopping: '$(debug-stop) Vizsla Stopping',
    stopped: '$(circle-slash) Vizsla Stopped',
    error: '$(error) Vizsla Error',
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
