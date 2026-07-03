import * as vscode from 'vscode';
import {
  type LanguageClientOptions,
  RevealOutputChannelOn,
} from 'vscode-languageclient';

import { createVideClientOptionsCore } from '../common/clientOptionsCore';
import { fileDocumentSelector } from '../common/documentSelector';

type ClientMiddleware = NonNullable<LanguageClientOptions['middleware']>;

export type NodeClientOptionsParams = {
  outputChannel: vscode.OutputChannel;
  trace: 'off' | 'messages' | 'verbose';
  provideRenameEdits: NonNullable<ClientMiddleware['provideRenameEdits']>;
};

export function createNodeClientOptions({
  outputChannel,
  trace,
  provideRenameEdits,
}: NodeClientOptionsParams): LanguageClientOptions {
  const coreOptions = createVideClientOptionsCore({
    documentSelector: fileDocumentSelector,
    configuration: vscode.workspace.getConfiguration('vide'),
    provideRenameEdits,
  });

  return {
    ...coreOptions,
    synchronize: {
      configurationSection: ['vide'],
    },
    outputChannel,
    traceOutputChannel: outputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    middleware: {
      ...coreOptions.middleware,
      provideReferences: async (document, position, options, token, next) => {
        options.includeDeclaration = includeDeclarationInReferences(document);
        return await next(document, position, options, token);
      },
    },
    ...(trace !== 'off' && { trace }),
  };
}

function includeDeclarationInReferences(document: vscode.TextDocument): boolean {
  return (
    vscode.workspace
      .getConfiguration('vide', document)
      .get<boolean>('references.includeDeclaration') ?? true
  );
}
