import * as vscode from 'vscode';

import { PROJECT_CONFIG_FILE_NAME } from './projectConfigCommon';
import {
  asProjectStatus,
  getVideStatusPresentation,
  initialProjectStatus,
  type LanguageStatusPresentation,
  type ProjectStatus,
  type ProjectStatusMessages,
  type FuseSocCoreSelection,
  type ServerStatus,
  type ServerStatusMessages,
  type VideStatusMessages,
} from './status';

const statusBarPriority = 101;

export const reloadWorkspaceCommand = 'vide.reloadWorkspace';
export const showOutputCommand = 'vide.showOutput';
export const showStatusCommand = 'vide.showStatus';
export const reloadWorkspaceRequest = 'vide.server.reloadWorkspace';
export const selectFuseSocProjectRequest = 'vide.server.selectFuseSocProject';
export const listFuseSocTargetsRequest = 'vide.server.listFuseSocTargets';
export const projectStatusNotification = 'vide/projectStatus';

export interface VideStatusActions {
  createManifest: (rootUris: readonly string[]) => Promise<void>;
  selectFuseSocCore?: (workspaceUri: string, coreUri: string) => Promise<void>;
  profileDiagnostics?: () => Promise<void>;
  reloadProject: () => Promise<void>;
  restartServer: () => Promise<void>;
  showOutput: () => void;
  log: (message: string) => void;
}

export class VideStatusController implements vscode.Disposable {
  private readonly item: vscode.StatusBarItem;
  private projectStatus = initialProjectStatus();
  private serverStatus: ServerStatus = 'stopped';
  private serverDetail: string | undefined;
  private readonly pendingCoreSelections = new Set<string>();

  constructor(private readonly actions: VideStatusActions) {
    this.item = vscode.window.createStatusBarItem(
      'vide.status',
      vscode.StatusBarAlignment.Right,
      statusBarPriority,
    );
    this.item.name = vscode.l10n.t('Vide');
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
    if (status.fusesocCoreSelections?.some((selection) => selection.coreUris.length > 1)) {
      void this.promptForFuseSocCoreSelections(status.fusesocCoreSelections);
    }
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
      title: vscode.l10n.t('Vide Status'),
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
      case 'selectFuseSocCore':
        await this.promptForFuseSocCoreSelections(status.fusesocCoreSelections ?? []);
        break;
      case 'profileDiagnostics':
        await this.actions.profileDiagnostics?.();
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
      title: vscode.l10n.t('Show Vide Status'),
      command: showStatusCommand,
    };
  }

  private currentPresentation(): LanguageStatusPresentation {
    return getVideStatusPresentation(
      {
        serverStatus: this.serverStatus,
        serverDetail: this.serverDetail,
        projectStatus: this.projectStatus,
      },
      localizedVideStatusMessages(),
    );
  }

  private quickPickItems(status: ProjectStatus): VideStatusQuickPickItem[] {
    const items: VideStatusQuickPickItem[] = [];

    if (status.errors.length > 0) {
      items.push({
        label: vscode.l10n.t('$(error) Project Configuration Error'),
        description: status.errors[0],
        action: 'showOutput',
      });
    }

    if (status.fusesocCoreSelections?.some((selection) => selection.coreUris.length > 1)) {
      items.push({
        label: vscode.l10n.t('$(list-selection) Select FuseSoC Root Core'),
        description: vscode.l10n.t(
          'Choose the root core before loading the FuseSoC project',
        ),
        action: 'selectFuseSocCore',
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

    if (this.actions.profileDiagnostics) {
      items.push({
        label: vscode.l10n.t('$(pulse) Profile Diagnostics'),
        description: vscode.l10n.t('Measure current-file or workspace diagnostics performance'),
        action: 'profileDiagnostics',
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
        description: vscode.l10n.t('Restart Vide if the server process is unhealthy'),
        action: 'restartServer',
      },
      {
        label: vscode.l10n.t('$(output) Show Output'),
        description: vscode.l10n.t('Open the Vide language server log'),
        action: 'showOutput',
      },
    );

    return items;
  }

  private async promptForFuseSocCoreSelections(
    selections: readonly FuseSocCoreSelection[],
  ): Promise<void> {
    const action = this.actions.selectFuseSocCore;
    if (!action) {
      this.actions.log(
        '[ERROR] FuseSoC core selection was requested but this client does not support it.',
      );
      return;
    }

    for (const selection of selections) {
      const key = `${selection.workspaceUri}\0${selection.coreUris.join('\0')}`;
      if (this.pendingCoreSelections.has(key)) {
        continue;
      }
      this.pendingCoreSelections.add(key);

      try {
        let coreUri: string | undefined;
        if (selection.coreUris.length === 1) {
          coreUri = selection.coreUris[0];
        } else {
          const selected = await vscode.window.showQuickPick(
            selection.coreUris.map((uri) => ({
              label: baseName(uriDisplayPath(uri)),
              description: uriDisplayPath(uri),
              uri,
            })),
            {
              title: vscode.l10n.t('Select FuseSoC Root Core'),
              placeHolder: vscode.l10n.t(
                'Multiple .core files were found; choose the project root core',
              ),
            },
          );
          if (!selected) {
            return;
          }
          coreUri = selected.uri;
        }
        await action(selection.workspaceUri, coreUri);
      } catch (error) {
        const message = vscode.l10n.t(
          'Failed to persist the FuseSoC root core: {0}',
          error instanceof Error ? error.message : String(error),
        );
        this.actions.log(`[ERROR] ${message}`);
        void vscode.window.showErrorMessage(message);
      } finally {
        this.pendingCoreSelections.delete(key);
      }
    }
  }
}

type VideStatusQuickPickItem = vscode.QuickPickItem & {
  action:
    | 'openManifest'
    | 'createManifest'
    | 'selectFuseSocCore'
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
    text: vscode.l10n.t('Vide'),
    startingDetail: vscode.l10n.t('Vide language server is starting.'),
    readyDetail: vscode.l10n.t('Vide language server is running.'),
    stoppingDetail: vscode.l10n.t('Vide language server is stopping.'),
    stoppedDetail: vscode.l10n.t('Vide language server is stopped.'),
    errorDetail: vscode.l10n.t('Vide language server failed.'),
  };
}

function localizedVideStatusMessages(): VideStatusMessages {
  return {
    server: localizedServerStatusMessages(),
    project: localizedProjectStatusMessages(),
  };
}

function localizedProjectStatusMessages(): ProjectStatusMessages {
  return {
    text: vscode.l10n.t('Vide'),
    loadingDetail: vscode.l10n.t('Loading project configuration'),
    loadedOneManifestDetail: vscode.l10n.t('Project manifest loaded'),
    loadedManyManifestsDetail: (count) =>
      vscode.l10n.t('{0} project manifests loaded', count),
    selectionRequiredDetail: vscode.l10n.t('Select the FuseSoC project core and target'),
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
    vscode.window.showWarningMessage(vscode.l10n.t('No Vide project manifest is loaded.'));
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
        label: baseName(displayPath),
        description: displayPath,
        uri,
      };
    }),
    {
      title: vscode.l10n.t('Open Vide Project Manifest'),
    },
  );
  if (!selected) {
    return;
  }

  await openUri(selected.uri);
}

function baseName(value: string): string {
  const normalized = value.replace(/\\/g, '/').replace(/\/+$/, '');
  const slashIndex = normalized.lastIndexOf('/');
  return slashIndex >= 0 ? normalized.slice(slashIndex + 1) : normalized;
}
