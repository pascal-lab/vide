import type * as vscode from 'vscode';

export type LspMessage = Record<string, unknown>;

export interface ServerLaunch {
  command: string;
  args: string[];
  additionalArgs: string[];
  cwd: string;
}

export interface ProfilingDependencies {
  resolveLaunch(): ServerLaunch;
  createEnv(logLevel?: 'info' | 'debug', backtrace?: '1' | 'full'): NodeJS.ProcessEnv;
}

export type ProfileScope = 'workspace' | 'document';

type ProfileBaseTarget = {
  workspaceRoot: string;
  workspaceName: string;
};

export type ProfileTarget =
  | (ProfileBaseTarget & {
      scope: 'workspace';
    })
  | (ProfileBaseTarget & {
      scope: 'document';
      document: vscode.TextDocument;
    });

export type ProfileArtifacts = {
  dir: string;
  trace: string;
  summary: string;
  folded: string;
  html: string;
  svg: string;
  log: string;
};

export type ProfileRunSummary = {
  scope: ProfileScope;
  request: 'workspace/diagnostic' | 'textDocument/diagnostic';
  file?: string;
  workspace: string;
  elapsed_ms: number;
  diagnostic_request_ms: number;
  diagnostics: Record<string, unknown>;
  artifacts: {
    trace: string;
    folded: string;
    flamegraph_html: string;
    flamegraph: string;
    server_log: string;
  };
  trace_summary: Record<string, unknown>;
};
