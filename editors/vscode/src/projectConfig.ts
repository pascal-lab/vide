import * as path from 'node:path';

export const PROJECT_CONFIG_FILE_NAME = 'vizsla_config.toml';

export const DEFAULT_PROJECT_CONFIG_TEXT = `sources = []
include_dirs = []
`;

export function getProjectConfigPath(workspaceFolderPath: string): string {
  return path.join(workspaceFolderPath, PROJECT_CONFIG_FILE_NAME);
}
