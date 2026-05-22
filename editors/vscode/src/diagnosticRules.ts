export type DiagnosticRuleSeverity = 'ignore' | 'info' | 'warning' | 'error' | 'fatal';

export interface DiagnosticRule {
  selector: string;
  severity: DiagnosticRuleSeverity;
  force?: boolean;
}

export type DiagnosticRuleTarget = 'user' | 'workspace';

export interface DiagnosticLike {
  source?: string;
  code?: unknown;
  data?: unknown;
}

const slangDiagnosticSource = 'slang';

export function diagnosticCodeSelector(diagnostic: DiagnosticLike): string | undefined {
  if (diagnostic.source !== slangDiagnosticSource) {
    return undefined;
  }

  const code = diagnosticCode(diagnostic.code);
  return code ? `code:${code.subsystem}:${code.code}` : undefined;
}

export function diagnosticSelectorLabel(selector: string): string {
  return selector.startsWith('code:') ? 'this diagnostic type' : selector;
}

export function upsertDiagnosticRule(
  rules: readonly DiagnosticRule[],
  selector: string,
  severity: DiagnosticRuleSeverity,
): DiagnosticRule[] {
  let replaced = false;
  const next = rules.map((rule) => {
    if (rule.selector !== selector) {
      return rule;
    }

    replaced = true;
    return { ...rule, severity };
  });

  if (!replaced) {
    next.push({ selector, severity });
  }

  return next;
}

function diagnosticCode(code: unknown): { subsystem: number; code: number } | undefined {
  if (typeof code === 'string') {
    return parseDiagnosticCode(code);
  }

  if (isDiagnosticCodeObject(code)) {
    const value = code.value;
    if (typeof value === 'string') {
      return parseDiagnosticCode(value);
    }
  }

  return undefined;
}

function parseDiagnosticCode(value: string): { subsystem: number; code: number } | undefined {
  const [subsystem, code, ...rest] = value.split(':');
  if (rest.length > 0 || !subsystem || !code) {
    return undefined;
  }

  const parsedSubsystem = Number(subsystem);
  const parsedCode = Number(code);
  if (
    !Number.isInteger(parsedSubsystem)
    || !Number.isInteger(parsedCode)
    || parsedSubsystem < 0
    || parsedCode < 0
  ) {
    return undefined;
  }

  return { subsystem: parsedSubsystem, code: parsedCode };
}

function isDiagnosticCodeObject(value: unknown): value is { value: unknown } {
  return typeof value === 'object' && value !== null && 'value' in value;
}
