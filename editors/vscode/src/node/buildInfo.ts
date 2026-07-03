import * as fs from 'node:fs';
import * as path from 'node:path';

import * as vscode from 'vscode';

interface ExtensionBuildInfo {
  kind?: string;
  commitHash?: string;
  buildDate?: string;
  profileTrace?: boolean;
}

export function extensionBuildLabel(context: vscode.ExtensionContext): string {
  const version = extensionVersion(context);
  const buildInfo = extensionBuildInfo(context);
  const details = [buildInfo?.kind, buildInfo?.commitHash, buildInfo?.buildDate].filter(
    (part): part is string => typeof part === 'string' && part.length > 0,
  );
  return details.length > 0 ? `${version} (${details.join(', ')})` : version;
}

export function isProfileTraceEnabled(context: vscode.ExtensionContext): boolean {
  return extensionBuildInfo(context)?.profileTrace === true;
}

function extensionVersion(context: vscode.ExtensionContext): string {
  const packageJson = context.extension.packageJSON as { version?: unknown };
  return typeof packageJson.version === 'string' && packageJson.version.length > 0
    ? packageJson.version
    : 'unknown';
}

function extensionBuildInfo(context: vscode.ExtensionContext): ExtensionBuildInfo | undefined {
  const buildInfoPath = path.join(context.extensionPath, 'build-info.json');
  if (!fs.existsSync(buildInfoPath)) {
    return undefined;
  }
  return JSON.parse(fs.readFileSync(buildInfoPath, 'utf8')) as ExtensionBuildInfo;
}
