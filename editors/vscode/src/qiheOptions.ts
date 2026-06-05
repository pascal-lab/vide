import { execFile } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { promisify } from 'node:util';

import * as vscode from 'vscode';

import { resolvedQiheCommand } from './qiheCommand';

const execFileAsync = promisify(execFile);

export const generateQiheOptionsCommand = 'vide.generateQiheOptions';

const qiheOptionsFileName = 'qihe-options.toml';
const qiheOptionsRunPath = `./${qiheOptionsFileName}`;
const qiheGenOptionsArgs = ['gen-options', '-a', '-o', qiheOptionsRunPath];
const qiheGenOptionsTimeoutMs = 30000;

type QiheOptionsCommandDeps = {
  logQihe: (message: string) => void;
  showQiheErrorMessage: (message: string) => Promise<void>;
};

export function registerQiheOptionsCommand(deps: QiheOptionsCommandDeps): vscode.Disposable {
  return vscode.commands.registerCommand(generateQiheOptionsCommand, async (resource) => {
    await generateQiheOptions(resource, deps);
  });
}

async function generateQiheOptions(
  resource: unknown,
  deps: QiheOptionsCommandDeps,
): Promise<void> {
  const workspaceFolder = await qiheOptionsWorkspaceFolder(resource);
  if (!workspaceFolder) {
    return;
  }

  const optionsPath = path.join(workspaceFolder.uri.fsPath, qiheOptionsFileName);
  const optionsUri = vscode.Uri.file(optionsPath);
  const openOptionsAction = vscode.l10n.t('Open Options');

  if (fs.existsSync(optionsPath)) {
    const selection = await vscode.window.showInformationMessage(
      vscode.l10n.t('{0} already exists in {1}.', qiheOptionsFileName, workspaceFolder.name),
      openOptionsAction,
    );
    if (selection === openOptionsAction) {
      await openQiheOptions(optionsUri, deps);
    }
    return;
  }

  const command = resolvedQiheCommand(vscode.workspace.getConfiguration('vide'));
  deps.logQihe(`[INFO] Generating ${qiheOptionsFileName} in ${workspaceFolder.uri.fsPath}`);
  deps.logQihe(`[INFO] Running: ${command} ${qiheGenOptionsArgs.join(' ')}`);

  try {
    const { stdout, stderr } = await execFileAsync(command, qiheGenOptionsArgs, {
      cwd: workspaceFolder.uri.fsPath,
      env: process.env,
      shell: shouldExecuteWithShell(command),
      timeout: qiheGenOptionsTimeoutMs,
      windowsHide: true,
    });
    logProcessOutput(deps, 'qihe gen-options', stdout, stderr);

    if (!fs.existsSync(optionsPath)) {
      throw new Error(
        vscode.l10n.t('{0} completed without creating {1}.', command, qiheOptionsFileName),
      );
    }

    const selection = await vscode.window.showInformationMessage(
      vscode.l10n.t('Created {0} in {1}.', qiheOptionsFileName, workspaceFolder.name),
      openOptionsAction,
    );
    if (selection === openOptionsAction) {
      await openQiheOptions(optionsUri, deps);
    }
  } catch (error) {
    logProcessErrorOutput(deps, error);
    const message = vscode.l10n.t(
      'Failed to generate {0}: {1}',
      qiheOptionsFileName,
      (error as Error).message,
    );
    deps.logQihe(`[ERROR] ${message}`);
    await deps.showQiheErrorMessage(message);
  }
}

function commandResourceUri(resource: unknown): vscode.Uri | undefined {
  if (resource instanceof vscode.Uri) {
    return resource;
  }
  return vscode.window.activeTextEditor?.document.uri;
}

async function qiheOptionsWorkspaceFolder(
  resource: unknown,
): Promise<vscode.WorkspaceFolder | undefined> {
  const targetUri = commandResourceUri(resource);
  if (targetUri) {
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(targetUri);
    if (workspaceFolder && workspaceFolder.uri.scheme === 'file') {
      return workspaceFolder;
    }
  }

  const localWorkspaceFolders = (vscode.workspace.workspaceFolders ?? []).filter(
    (folder) => folder.uri.scheme === 'file',
  );
  if (localWorkspaceFolders.length === 1) {
    return localWorkspaceFolders[0];
  }

  if (localWorkspaceFolders.length === 0) {
    vscode.window.showWarningMessage(
      vscode.l10n.t('Open a local workspace folder before generating {0}.', qiheOptionsFileName),
    );
    return undefined;
  }

  const picked = await vscode.window.showQuickPick(
    localWorkspaceFolders.map((folder) => ({
      label: folder.name,
      description: folder.uri.fsPath,
      folder,
    })),
    { placeHolder: vscode.l10n.t('Select a workspace folder for {0}', qiheOptionsFileName) },
  );
  return picked?.folder;
}

async function openQiheOptions(
  uri: vscode.Uri,
  deps: QiheOptionsCommandDeps,
): Promise<void> {
  try {
    await vscode.window.showTextDocument(uri);
  } catch (error) {
    deps.logQihe(`[WARN] Failed to open ${uri.fsPath}: ${(error as Error).message}`);
  }
}

function shouldExecuteWithShell(command: string): boolean {
  return (
    process.platform === 'win32' &&
    ['.bat', '.cmd'].includes(path.extname(command).toLowerCase())
  );
}

function logProcessOutput(
  deps: QiheOptionsCommandDeps,
  label: string,
  stdout: string | Buffer,
  stderr: string | Buffer,
): void {
  const stdoutText = processOutputText(stdout).trimEnd();
  if (stdoutText.length > 0) {
    deps.logQihe(`[INFO] ${label} stdout:\n${stdoutText}`);
  }

  const stderrText = processOutputText(stderr).trimEnd();
  if (stderrText.length > 0) {
    deps.logQihe(`[INFO] ${label} stderr:\n${stderrText}`);
  }
}

function logProcessErrorOutput(deps: QiheOptionsCommandDeps, error: unknown): void {
  const failedProcess = error as { stdout?: unknown; stderr?: unknown };
  if (typeof failedProcess.stdout === 'string' || Buffer.isBuffer(failedProcess.stdout)) {
    const stdoutText = processOutputText(failedProcess.stdout).trimEnd();
    if (stdoutText.length > 0) {
      deps.logQihe(`[ERROR] qihe gen-options stdout:\n${stdoutText}`);
    }
  }

  if (typeof failedProcess.stderr === 'string' || Buffer.isBuffer(failedProcess.stderr)) {
    const stderrText = processOutputText(failedProcess.stderr).trimEnd();
    if (stderrText.length > 0) {
      deps.logQihe(`[ERROR] qihe gen-options stderr:\n${stderrText}`);
    }
  }
}

function processOutputText(output: string | Buffer): string {
  return Buffer.isBuffer(output) ? output.toString('utf8') : output;
}
