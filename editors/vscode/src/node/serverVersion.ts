import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

import * as vscode from 'vscode';

import { extensionBuildLabel } from './buildInfo';
import {
  createServerEnv,
  readConfiguration,
  resolveServerLaunch,
} from './serverLaunch';

type Logger = (message: string) => void;

const execFileAsync = promisify(execFile);
const versionTimeoutMs = 5000;

export async function showServerVersion(
  context: vscode.ExtensionContext,
  {
    log,
    showLanguageServerErrorMessage,
  }: {
    log: Logger;
    showLanguageServerErrorMessage: (message: string) => Promise<void>;
  },
): Promise<void> {
  try {
    const config = readConfiguration();
    const launch = resolveServerLaunch(context, config, log);
    const versionArgs = [...launch.args, '--version'];
    log(`[INFO] Checking server version: ${launch.command} ${versionArgs.join(' ')}`);
    const { stdout, stderr } = await execFileAsync(launch.command, versionArgs, {
      cwd: launch.cwd,
      env: createServerEnv(),
      timeout: versionTimeoutMs,
    });
    const output = `${stdout}${stderr}`.trim() || vscode.l10n.t('No version output');
    const firstLine = output.split(/\r?\n/, 1)[0] ?? output;
    log(`[INFO] Server version output:\n${output}`);
    vscode.window.showInformationMessage(
      vscode.l10n.t(
        'Vide extension: {0}; server: {1}',
        extensionBuildLabel(context),
        firstLine,
      ),
    );
  } catch (error) {
    const message = vscode.l10n.t(
      'Failed to query Vide server version: {0}',
      (error as Error).message,
    );
    log(`[ERROR] ${message}`);
    await showLanguageServerErrorMessage(message);
  }
}
