import * as vscode from 'vscode';
import {
  type LanguageClientOptions,
  RevealOutputChannelOn,
} from 'vscode-languageclient';

import { serverInitializationOptions } from './initializationOptions';

type ClientMiddleware = NonNullable<LanguageClientOptions['middleware']>;

export type NodeClientOptionsParams = {
  outputChannel: vscode.OutputChannel;
  trace: 'off' | 'messages' | 'verbose';
  provideRenameEdits: NonNullable<ClientMiddleware['provideRenameEdits']>;
};

export const fileDocumentSelector: LanguageClientOptions['documentSelector'] = [
  { scheme: 'file', language: 'verilog' },
  { scheme: 'file', language: 'systemverilog' },
];

export function createNodeClientOptions({
  outputChannel,
  trace,
  provideRenameEdits,
}: NodeClientOptionsParams): LanguageClientOptions {
  return {
    documentSelector: fileDocumentSelector,
    synchronize: {
      configurationSection: ['vide'],
    },
    outputChannel,
    traceOutputChannel: outputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
    initializationOptions: serverInitializationOptions(vscode.workspace.getConfiguration('vide')),
    middleware: {
      provideReferences: async (document, position, options, token, next) => {
        options.includeDeclaration = includeDeclarationInReferences(document);
        return await next(document, position, options, token);
      },
      provideRenameEdits,
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
