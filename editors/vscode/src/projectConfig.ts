import * as path from 'node:path';

import {
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
} from './generated/projectConfigSchema';

export {
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
} from './generated/projectConfigSchema';

export const PROJECT_CONFIG_FILE_NAME = 'vizsla.toml';
export const LEGACY_PROJECT_CONFIG_FILE_NAME = 'vizsla_config.toml';
export const PROJECT_CONFIG_FILE_NAMES = [
  PROJECT_CONFIG_FILE_NAME,
  LEGACY_PROJECT_CONFIG_FILE_NAME,
] as const;
export const PROJECT_SOURCE_FILE_EXTENSIONS = [
  '.v',
  '.sv',
  '.vh',
  '.svh',
  '.svi',
] as const;
export const PROJECT_SOURCE_FILE_GLOB = '**/*.{v,sv,vh,svh,svi}';

export const DEFAULT_PROJECT_CONFIG_TEXT = `#:schema ${PROJECT_CONFIG_SCHEMA_URL}
# Syntax-only startup config. Keep these arrays empty to avoid scanning the workspace.
# Fill real paths, for example sources = ["rtl"] and include_dirs = ["include"], to enable semantic diagnostics.
sources = []
include_dirs = []
`;

export function isProjectConfigFileName(fileName: string): boolean {
  return PROJECT_CONFIG_FILE_NAMES.includes(
    fileName as (typeof PROJECT_CONFIG_FILE_NAMES)[number],
  );
}

export function isProjectSourceFileName(fileName: string): boolean {
  return PROJECT_SOURCE_FILE_EXTENSIONS.includes(
    path.extname(fileName).toLowerCase() as (typeof PROJECT_SOURCE_FILE_EXTENSIONS)[number],
  );
}

export function getProjectConfigPath(
  workspaceFolderPath: string,
  fileName = PROJECT_CONFIG_FILE_NAME,
): string {
  return path.join(workspaceFolderPath, fileName);
}
