import type * as vscode from 'vscode';

export const TOML_LANGUAGE_ID = 'toml';

const SYNTAX_TOKEN_COMMENT = 1 as vscode.SyntaxTokenType;
const SYNTAX_TOKEN_STRING = 2 as vscode.SyntaxTokenType;

export const TOML_LANGUAGE_CONFIGURATION: vscode.LanguageConfiguration = {
  brackets: [['[', ']']],
  autoClosingPairs: [
    { open: '[', close: ']' },
    { open: '"', close: '"', notIn: [SYNTAX_TOKEN_STRING, SYNTAX_TOKEN_COMMENT] },
  ],
};
