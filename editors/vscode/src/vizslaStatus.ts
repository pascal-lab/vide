import * as path from 'node:path';

import * as vscode from 'vscode';

import { PROJECT_CONFIG_FILE_NAME } from './projectConfig';
import {
  asProjectStatus,
  getVizslaStatusPresentation,
  initialProjectStatus,
  type LanguageStatusPresentation,
  type ProjectStatus,
  type ProjectStatusMessages,
  type ServerStatus,
  type ServerStatusMessages,
  type VizslaStatusMessages,
} from './status';

const statusBarPriority = 101;

export const reloadWorkspaceCommand = 'vizsla.reloadWorkspace';
export const showOutputCommand = 'vizsla.showOutput';
export const showStatusCommand = 'vizsla.showStatus';
export const reloadWorkspaceRequest = 'vizsla.server.reloadWorkspace';
export const projectStatusNotification = 'vizsla/projectStatus';

export interface VizslaStatusActions {
  createManifest: (rootUris: readonly string[]) => Promise<void>;
  profileDiagnostics: () => Promise<void>;
  reloadProject: () => Promise<void>;
  restartServer: () => Promise<void>;
  showOutput: () => void;
  log: (message: string) => void;
}

export class VizslaStatusController implements vscode.Disposable {
  private readonly item: vscode.StatusBarItem;
  private projectStatus = initialProjectStatus();
  private serverStatus: ServerStatus = 'stopped';
  private serverDetail: string | undefined;

  constructor(private readonly actions: VizslaStatusActions) {
    this.item = vscode.window.createStatusBarItem(
      'vizsla.status',
      vscode.StatusBarAlignment.Right,
      statusBarPriority,
    );
    this.item.name = vscode.l10n.t('Vizsla');
    this.item.command = this.command();
    this.update();
  }

  dispose(): void {
    this.item.dispose();
  }

  handleProjectNotification(params: unknown): void {
    const status = asProjectStatus(params);
    if (!status) {
      this.actions.log(
        `[WARN] Ignoring malformed project status notification: ${JSON.stringify(params)}`,
      );
      return;
    }

    this.updateProjectStatus(status);
  }

  updateProjectStatus(status: ProjectStatus): void {
    this.projectStatus = status;
    this.update();
  }

  updateServerStatus(status: ServerStatus, detail?: string): void {
    this.serverStatus = status;
    this.serverDetail = detail;
    this.update();
  }

  private update(): void {
    const presentation = this.currentPresentation();
    this.item.text = statusBarText(presentation);
    this.item.tooltip = presentation.detail;
    this.item.backgroundColor = statusBarBackgroundColor(presentation.severity);
    this.item.command = this.command();
    this.item.show();
  }

  async show(): Promise<void> {
    const status = this.projectStatus;
    const presentation = this.currentPresentation();
    const items = this.quickPickItems(status);

    const selected = await vscode.window.showQuickPick(items, {
      title: vscode.l10n.t('Vizsla Status'),
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
        await this.actions.createManifest(status.unconfiguredRootUris);
        break;
      case 'profileDiagnostics':
        await this.actions.profileDiagnostics();
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
      title: vscode.l10n.t('Show Vizsla Status'),
      command: showStatusCommand,
    };
  }

  private currentPresentation(): LanguageStatusPresentation {
    return getVizslaStatusPresentation(
      {
        serverStatus: this.serverStatus,
        serverDetail: this.serverDetail,
        projectStatus: this.projectStatus,
      },
      localizedVizslaStatusMessages(),
    );
  }

  private quickPickItems(status: ProjectStatus): VizslaStatusQuickPickItem[] {
    const items: VizslaStatusQuickPickItem[] = [];

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
        label: vscode.l10n.t('$(pulse) Profile Diagnostics'),
        description: vscode.l10n.t('Measure current-file or workspace diagnostics performance'),
        action: 'profileDiagnostics',
      },
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

type VizslaStatusQuickPickItem = vscode.QuickPickItem & {
  action:
    | 'openManifest'
    | 'createManifest'
    | 'profileDiagnostics'
    | 'reloadProject'
    | 'restartServer'
    | 'showOutput';
};

function statusBarText(presentation: LanguageStatusPresentation): string {
  if (presentation.busy) {
    return `$(sync~spin) ${presentation.text}`;
  }

  switch (presentation.severity) {
    case 'error':
      return `$(error) ${presentation.text}`;
    case 'warning':
      return `$(warning) ${presentation.text}`;
    case 'information':
      return presentation.text;
  }
}

function statusBarBackgroundColor(
  severity: LanguageStatusPresentation['severity'],
): vscode.ThemeColor | undefined {
  switch (severity) {
    case 'error':
      return new vscode.ThemeColor('statusBarItem.errorBackground');
    case 'warning':
      return new vscode.ThemeColor('statusBarItem.warningBackground');
    case 'information':
      return undefined;
  }
}

function localizedServerStatusMessages(): ServerStatusMessages {
  return {
    text: vscode.l10n.t('Vizsla'),
    startingDetail: vscode.l10n.t('Vizsla language server is starting.'),
    readyDetail: vscode.l10n.t('Vizsla language server is running.'),
    stoppingDetail: vscode.l10n.t('Vizsla language server is stopping.'),
    stoppedDetail: vscode.l10n.t('Vizsla language server is stopped.'),
    errorDetail: vscode.l10n.t('Vizsla language server failed.'),
  };
}

function localizedVizslaStatusMessages(): VizslaStatusMessages {
  return {
    server: localizedServerStatusMessages(),
    project: localizedProjectStatusMessages(),
  };
}

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
