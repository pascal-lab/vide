import * as path from 'node:path';

export const PROJECT_CONFIG_FILE_NAME = 'vizsla.toml';
export const LEGACY_PROJECT_CONFIG_FILE_NAME = 'vizsla_config.toml';
export const PROJECT_CONFIG_FILE_NAMES = [
  PROJECT_CONFIG_FILE_NAME,
  LEGACY_PROJECT_CONFIG_FILE_NAME,
] as const;
export const PROJECT_CONFIG_DOCUMENT_SELECTORS = PROJECT_CONFIG_FILE_NAMES.map((fileName) => ({
  scheme: 'file',
  pattern: `**/${fileName}`,
}));

export const DEFAULT_PROJECT_CONFIG_TEXT = `# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
`;

export function getProjectConfigPath(workspaceFolderPath: string): string {
  return path.join(workspaceFolderPath, PROJECT_CONFIG_FILE_NAME);
}

export function getProjectConfigPaths(workspaceFolderPath: string): string[] {
  return PROJECT_CONFIG_FILE_NAMES.map((fileName) => path.join(workspaceFolderPath, fileName));
}
