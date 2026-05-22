import * as vscode from 'vscode';

import { toLanguageStatusSeverity, vizslaLanguageSelector } from './languageStatus';
import {
  getServerStatusPresentation,
  type ServerStatus,
  type ServerStatusMessages,
} from './status';

export const showOutputCommand = 'vizsla.showOutput';

export class ServerStatusController implements vscode.Disposable {
  private readonly item: vscode.LanguageStatusItem;

  constructor() {
    this.item = vscode.languages.createLanguageStatusItem(
      'vizsla.serverStatus',
      vizslaLanguageSelector,
    );
    this.item.name = vscode.l10n.t('Vizsla Language Server');
    this.item.command = this.command();
    this.update('stopped');
  }

  dispose(): void {
    this.item.dispose();
  }

  update(status: ServerStatus, detail?: string): void {
    const presentation = getServerStatusPresentation(
      status,
      detail,
      localizedServerStatusMessages(),
    );
    this.item.text = presentation.text;
    this.item.detail = presentation.detail;
    this.item.busy = presentation.busy;
    this.item.severity = toLanguageStatusSeverity(presentation.severity);
    this.item.command = this.command();
  }

  private command(): vscode.Command {
    return {
      title: vscode.l10n.t('Show Output'),
      command: showOutputCommand,
    };
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
