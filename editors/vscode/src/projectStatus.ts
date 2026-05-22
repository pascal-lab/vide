import * as path from 'node:path';

import * as vscode from 'vscode';

import { PROJECT_CONFIG_FILE_NAME } from './projectConfig';
import {
  asProjectStatus,
  getProjectStatusPresentation,
  projectStatusFallback,
  type ProjectStatus,
  type ProjectStatusMessages,
} from './status';

export const reloadWorkspaceCommand = 'vizsla.reloadWorkspace';
export const showProjectStatusCommand = 'vizsla.showProjectStatus';
export const reloadWorkspaceRequest = 'vizsla.server.reloadWorkspace';
export const projectStatusNotification = 'vizsla/projectStatus';

export interface ProjectStatusActions {
  createManifest: () => Promise<void>;
  reloadProject: () => Promise<void>;
  restartServer: () => Promise<void>;
  showOutput: () => void;
  log: (message: string) => void;
}

export class ProjectStatusController implements vscode.Disposable {
  private readonly item: vscode.LanguageStatusItem;
  private status = projectStatusFallback();

  constructor(private readonly actions: ProjectStatusActions) {
    this.item = vscode.languages.createLanguageStatusItem('vizsla.projectStatus', [
      { scheme: 'file', language: 'verilog' },
      { scheme: 'file', language: 'systemverilog' },
    ]);
    this.item.name = vscode.l10n.t('Vizsla Project');
    this.item.command = this.command();
    this.update(this.status);
  }

  dispose(): void {
    this.item.dispose();
  }

  handleNotification(params: unknown): void {
    const status = asProjectStatus(params);
    if (!status) {
      this.actions.log(
        `[WARN] Ignoring malformed project status notification: ${JSON.stringify(params)}`,
      );
      return;
    }

    this.update(status);
  }

  update(status: ProjectStatus): void {
    this.status = status;

    const presentation = getProjectStatusPresentation(status, localizedProjectStatusMessages());
    this.item.text = presentation.text;
    this.item.detail = presentation.detail;
    this.item.busy = presentation.busy;
    this.item.severity = toLanguageStatusSeverity(presentation.severity);
    this.item.command = this.command();
  }

  async show(): Promise<void> {
    const status = this.status;
    const presentation = getProjectStatusPresentation(status, localizedProjectStatusMessages());
    const items = this.quickPickItems(status);

    const selected = await vscode.window.showQuickPick(items, {
      title: vscode.l10n.t('Vizsla Project Status'),
      placeHolder: presentation.detail,
    });
    if (!selected) {
      return;
    }

    switch (selected.action) {
      case 'openManifest':
        await openProjectManifest(status);
        break;
      case 'createManifest':
        await this.actions.createManifest();
        break;
      case 'reloadProject':
        await this.actions.reloadProject();
        break;
      case 'restartServer':
        await this.actions.restartServer();
        break;
      case 'showOutput':
        this.actions.showOutput();
        break;
    }
  }

  private command(): vscode.Command {
    return {
      title: vscode.l10n.t('Show Vizsla Project Status'),
      command: showProjectStatusCommand,
    };
  }

  private quickPickItems(status: ProjectStatus): ProjectStatusQuickPickItem[] {
    const items: ProjectStatusQuickPickItem[] = [];

    if (status.errors.length > 0) {
      items.push({
        label: vscode.l10n.t('$(error) Project Configuration Error'),
        description: status.errors[0],
        action: 'showOutput',
      });
    }

    if (status.manifestUris.length > 0) {
      items.push({
        label: vscode.l10n.t('$(go-to-file) Open Manifest'),
        description:
          status.manifestUris.length === 1
            ? uriDisplayPath(status.manifestUris[0])
            : vscode.l10n.t('{0} manifests', status.manifestUris.length),
        action: 'openManifest',
      });
    }

    if (status.state === 'none') {
      items.push({
        label: vscode.l10n.t('$(new-file) Create Manifest'),
        description: vscode.l10n.t(
          'Create {0} in missing workspace folders',
          PROJECT_CONFIG_FILE_NAME,
        ),
        action: 'createManifest',
      });
    }

    items.push(
      {
        label: vscode.l10n.t('$(refresh) Reload Project'),
        description: vscode.l10n.t('Refresh project manifests without restarting the server'),
        action: 'reloadProject',
      },
      {
        label: vscode.l10n.t('$(debug-restart) Restart Language Server'),
        description: vscode.l10n.t('Restart Vizsla if the server process is unhealthy'),
        action: 'restartServer',
      },
      {
        label: vscode.l10n.t('$(output) Show Output'),
        description: vscode.l10n.t('Open the Vizsla language server log'),
        action: 'showOutput',
      },
    );

    return items;
  }
}

type ProjectStatusQuickPickItem = vscode.QuickPickItem & {
  action: 'openManifest' | 'createManifest' | 'reloadProject' | 'restartServer' | 'showOutput';
};

function localizedProjectStatusMessages(): ProjectStatusMessages {
  return {
    text: vscode.l10n.t('Vizsla'),
    loadingDetail: vscode.l10n.t('Loading project configuration'),
    loadedOneManifestDetail: vscode.l10n.t('Project manifest loaded'),
    loadedManyManifestsDetail: (count) =>
      vscode.l10n.t('{0} project manifests loaded', count),
    noManifestDetail: vscode.l10n.t('No project manifest'),
    errorDetail: vscode.l10n.t('Project configuration failed'),
  };
}

function toLanguageStatusSeverity(
  severity: 'information' | 'warning' | 'error',
): vscode.LanguageStatusSeverity {
  switch (severity) {
    case 'information':
      return vscode.LanguageStatusSeverity.Information;
    case 'warning':
      return vscode.LanguageStatusSeverity.Warning;
    case 'error':
      return vscode.LanguageStatusSeverity.Error;
  }
}

function uriDisplayPath(uriString: string): string {
  try {
    return vscode.Uri.parse(uriString).fsPath || uriString;
  } catch {
    return uriString;
  }
}

async function openUri(uriString: string): Promise<void> {
  const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uriString));
  await vscode.window.showTextDocument(document);
}

async function openProjectManifest(status: ProjectStatus): Promise<void> {
  if (status.manifestUris.length === 0) {
    vscode.window.showWarningMessage(vscode.l10n.t('No Vizsla project manifest is loaded.'));
    return;
  }

  if (status.manifestUris.length === 1) {
    await openUri(status.manifestUris[0]);
    return;
  }

  const selected = await vscode.window.showQuickPick(
    status.manifestUris.map((uri) => {
      const displayPath = uriDisplayPath(uri);
      return {
        label: path.basename(displayPath),
        description: displayPath,
        uri,
      };
    }),
    {
      title: vscode.l10n.t('Open Vizsla Project Manifest'),
    },
  );
  if (!selected) {
    return;
  }

  await openUri(selected.uri);
}
