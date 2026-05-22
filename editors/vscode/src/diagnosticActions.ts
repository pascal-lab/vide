import * as vscode from 'vscode';

import {
  diagnosticCodeSelector,
  type DiagnosticRule,
  type DiagnosticRuleSeverity,
  type DiagnosticRuleTarget,
  upsertDiagnosticRule,
} from './diagnosticRules';

export const configureDiagnosticRuleCommand = 'vizsla.configureDiagnosticRule';

interface ConfigureDiagnosticRuleArgs {
  selector: string;
  severity: DiagnosticRuleSeverity;
  target: DiagnosticRuleTarget;
}

const diagnosticRulesSetting = 'diagnostics.slang.rules';
const documentSelector: vscode.DocumentSelector = [
  { scheme: 'file', language: 'verilog' },
  { scheme: 'file', language: 'systemverilog' },
];

export function registerDiagnosticActions(context: vscode.ExtensionContext): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(configureDiagnosticRuleCommand, configureDiagnosticRule),
  );

  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider(
      documentSelector,
      new DiagnosticRuleCodeActionProvider(),
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] },
    ),
  );
}

class DiagnosticRuleCodeActionProvider implements vscode.CodeActionProvider {
  provideCodeActions(
    document: vscode.TextDocument,
    _range: vscode.Range | vscode.Selection,
    context: vscode.CodeActionContext,
  ): vscode.CodeAction[] {
    const workspaceActionsEnabled = vscode.workspace.workspaceFolders !== undefined;
    const actions: vscode.CodeAction[] = [];
    const diagnosticsBySelector = new Map<string, vscode.Diagnostic>();

    for (const diagnostic of context.diagnostics) {
      const selector = diagnosticCodeSelector(diagnostic);
      if (!selector) {
        continue;
      }

      const existing = diagnosticsBySelector.get(selector);
      if (!existing || diagnostic.severity === vscode.DiagnosticSeverity.Error) {
        diagnosticsBySelector.set(selector, diagnostic);
      }
    }

    for (const [selector, diagnostic] of diagnosticsBySelector) {
      const canDowngrade = diagnostic.severity === vscode.DiagnosticSeverity.Error;

      if (workspaceActionsEnabled && vscode.workspace.getWorkspaceFolder(document.uri)) {
        actions.push(createDiagnosticRuleAction(selector, diagnostic, 'ignore', 'workspace'));
        if (canDowngrade) {
          actions.push(createDiagnosticRuleAction(selector, diagnostic, 'warning', 'workspace'));
        }
      }

      actions.push(createDiagnosticRuleAction(selector, diagnostic, 'ignore', 'user'));
      if (canDowngrade) {
        actions.push(createDiagnosticRuleAction(selector, diagnostic, 'warning', 'user'));
      }
    }

    return actions;
  }
}

function createDiagnosticRuleAction(
  selector: string,
  diagnostic: vscode.Diagnostic,
  severity: DiagnosticRuleSeverity,
  target: DiagnosticRuleTarget,
): vscode.CodeAction {
  const title = diagnosticRuleActionTitle(severity, target);
  const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
  action.diagnostics = [diagnostic];
  action.command = {
    command: configureDiagnosticRuleCommand,
    title,
    arguments: [{ selector, severity, target } satisfies ConfigureDiagnosticRuleArgs],
  };
  return action;
}

function diagnosticRuleActionTitle(
  severity: DiagnosticRuleSeverity,
  target: DiagnosticRuleTarget,
): string {
  if (target === 'workspace') {
    return severity === 'ignore'
      ? vscode.l10n.t('Ignore this diagnostic type in workspace settings')
      : vscode.l10n.t('Downgrade this diagnostic type to warning in workspace settings');
  }

  return severity === 'ignore'
    ? vscode.l10n.t('Ignore this diagnostic type in user settings')
    : vscode.l10n.t('Downgrade this diagnostic type to warning in user settings');
}

async function configureDiagnosticRule(args: ConfigureDiagnosticRuleArgs): Promise<void> {
  try {
    const config = vscode.workspace.getConfiguration('vizsla');
    const target =
      args.target === 'workspace'
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;
    const rules = diagnosticRulesForTarget(config, args.target);
    const nextRules = upsertDiagnosticRule(rules, args.selector, args.severity);

    await config.update(diagnosticRulesSetting, nextRules, target);
  } catch (error) {
    vscode.window.showErrorMessage(
      vscode.l10n.t(
        'Unable to update Vizsla diagnostic rules: {0}',
        (error as Error).message,
      ),
    );
  }
}

function diagnosticRulesForTarget(
  config: vscode.WorkspaceConfiguration,
  target: DiagnosticRuleTarget,
): DiagnosticRule[] {
  const inspected = config.inspect<DiagnosticRule[]>(diagnosticRulesSetting);
  const value = target === 'workspace' ? inspected?.workspaceValue : inspected?.globalValue;

  return Array.isArray(value) ? value : [];
}
