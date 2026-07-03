import type * as vscode from 'vscode';
import type {
  DocumentSelector as ProtocolDocumentSelector,
} from 'vscode-languageserver-protocol';

const videLanguageIds = ['verilog', 'systemverilog'] as const;

export type VideDocumentSelectorOptions = {
  scheme?: string;
};

export function createVideDocumentSelector(
  options: VideDocumentSelectorOptions = {},
): ProtocolDocumentSelector {
  return videLanguageIds.map((language) => (
    options.scheme ? { scheme: options.scheme, language } : { language }
  ));
}

export const fileDocumentSelector = createVideDocumentSelector({ scheme: 'file' });

export function createVideCodeDocumentSelector(
  options: VideDocumentSelectorOptions = {},
): vscode.DocumentSelector {
  return createVideDocumentSelector(options) as vscode.DocumentFilter[];
}

export const fileCodeDocumentSelector = createVideCodeDocumentSelector({ scheme: 'file' });
