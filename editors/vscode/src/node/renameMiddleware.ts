import * as vscode from 'vscode';
import type { LanguageClient, LanguageClientOptions } from 'vscode-languageclient/node';

type Logger = (message: string) => void;
type ClientMiddleware = NonNullable<LanguageClientOptions['middleware']>;

const renameExpansionInfoRequest = 'vide.server.renameExpansionInfo';
const expandedRenameRequest = 'vide.server.expandedRename';
const renameConflictInfoRequest = 'vide.server.renameConflictInfo';

type RenameExpansionInfo = {
  additionalSymbols: number;
};

type RenameConflictInfo = {
  conflicts: number;
};

export function createProvideExpandedRenameEdits(
  getClient: () => LanguageClient | undefined,
  log: Logger,
): NonNullable<ClientMiddleware['provideRenameEdits']> {
  return async (document, position, newName, token, next) => {
    const languageClient = getClient();
    if (!languageClient) {
      return await next(document, position, newName, token);
    }

    const textDocumentPosition = {
      textDocument: languageClient.code2ProtocolConverter.asTextDocumentIdentifier(document),
      position: languageClient.code2ProtocolConverter.asPosition(position),
    };
    const standardRename = async (): Promise<vscode.WorkspaceEdit | null | undefined> => {
      if (
        !(await confirmRenameCollision(
          languageClient,
          textDocumentPosition,
          newName,
          false,
          token,
        ))
      ) {
        return emptyRenameEdit();
      }
      return await next(document, position, newName, token);
    };

    let info: RenameExpansionInfo | undefined;
    try {
      info = await languageClient.sendRequest<RenameExpansionInfo>(
        'workspace/executeCommand',
        {
          command: renameExpansionInfoRequest,
          arguments: [{ textDocumentPosition }],
        },
        token,
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      log(`[WARN] Falling back to standard rename: ${message}`);
    }

    if (!info || info.additionalSymbols === 0) {
      return await standardRename();
    }

    const recursiveAction = vscode.l10n.t('Rename Connected Ports/Signals');
    const localAction = vscode.l10n.t('Only This Symbol');
    const selected = await vscode.window.showInformationMessage(
      vscode.l10n.t(
        'Rename {0} connected port/signal symbol(s) as well?',
        info.additionalSymbols,
      ),
      recursiveAction,
      localAction,
    );

    if (selected === localAction) {
      return await standardRename();
    }

    if (selected !== recursiveAction) {
      return emptyRenameEdit();
    }

    if (
      !(await confirmRenameCollision(
        languageClient,
        textDocumentPosition,
        newName,
        true,
        token,
      ))
    ) {
      return emptyRenameEdit();
    }

    const edit = await languageClient.sendRequest(
      'workspace/executeCommand',
      {
        command: expandedRenameRequest,
        arguments: [{ textDocumentPosition, newName }],
      },
      token,
    );
    return await languageClient.protocol2CodeConverter.asWorkspaceEdit(edit as never, token);
  };
}

function emptyRenameEdit(): vscode.WorkspaceEdit {
  return new vscode.WorkspaceEdit();
}

async function confirmRenameCollision(
  languageClient: LanguageClient,
  textDocumentPosition: unknown,
  newName: string,
  recursive: boolean,
  token: vscode.CancellationToken,
): Promise<boolean> {
  const info = await languageClient.sendRequest<RenameConflictInfo>(
    'workspace/executeCommand',
    {
      command: renameConflictInfoRequest,
      arguments: [{ textDocumentPosition, newName, recursive }],
    },
    token,
  );

  if (info.conflicts === 0) {
    return true;
  }

  const continueAction = vscode.l10n.t('Continue Rename');
  const cancelAction = vscode.l10n.t('Cancel');
  const selected = await vscode.window.showWarningMessage(
    vscode.l10n.t(
      'Renaming to "{0}" may collide with {1} existing symbol(s).',
      newName,
      info.conflicts,
    ),
    continueAction,
    cancelAction,
  );
  return selected === continueAction;
}
