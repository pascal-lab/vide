import * as vscode from 'vscode';

export const vizslaLanguageSelector: vscode.DocumentSelector = [
  { scheme: 'file', language: 'verilog' },
  { scheme: 'file', language: 'systemverilog' },
];

export function toLanguageStatusSeverity(
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
