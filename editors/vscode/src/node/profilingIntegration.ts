import * as vscode from 'vscode';

import { registerProfilingCommand } from '../profiling';
import {
  createServerEnv,
  readConfiguration,
  resolveServerLaunch,
} from './serverLaunch';

type Logger = (message: string) => void;

export function registerProfilingIntegration(
  context: vscode.ExtensionContext,
  {
    enabled,
    log,
  }: {
    enabled: boolean;
    log: Logger;
  },
): vscode.Disposable | undefined {
  if (!enabled) {
    return undefined;
  }

  return registerProfilingCommand(context, {
    resolveLaunch: () => resolveServerLaunch(context, readConfiguration(), log),
    createEnv: createServerEnv,
  });
}
