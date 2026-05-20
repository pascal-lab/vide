import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

export const TOMBI_EXTENSION_ID = 'tombi-toml.tombi';
export const TOMBI_SCHEMA_URL = 'https://pascal-lab.github.io/vizsla/schemas/vizsla.schema.json';

export type TombiSchemaInjectionScope = 'user' | 'workspace';
export type TombiSchemaInjectionResult = 'already-configured' | 'created' | 'updated';

const TOMBI_CONFIG_FILE_NAME = 'tombi.toml';
const TOMBI_DOT_CONFIG_FILE_NAME = '.tombi.toml';

const TOMBI_SCHEMA_CONFIG_BLOCK = `# Added by Vizsla. Provides schema-aware editing for vizsla.toml.
[[schemas]]
path = "${TOMBI_SCHEMA_URL}"
include = ["**/vizsla.toml", "**/vizsla_config.toml"]
`;

export function userTombiConfigPath(
  platform: NodeJS.Platform = process.platform,
  env: NodeJS.ProcessEnv = process.env,
  homeDir = os.homedir(),
): string {
  if (platform === 'win32') {
    const baseDir = env.APPDATA ?? path.win32.join(homeDir, 'AppData', 'Roaming');
    return path.win32.join(baseDir, 'tombi', 'config.toml');
  }

  if (platform === 'darwin') {
    return path.posix.join(homeDir, 'Library', 'Application Support', 'tombi', 'config.toml');
  }

  const baseDir = env.XDG_CONFIG_HOME ?? path.posix.join(homeDir, '.config');
  return path.posix.join(baseDir, 'tombi', 'config.toml');
}

export function workspaceTombiConfigPath(
  workspaceFolderPath: string,
  exists: (filePath: string) => boolean = fs.existsSync,
): string {
  const dotConfigPath = path.join(workspaceFolderPath, TOMBI_DOT_CONFIG_FILE_NAME);
  if (exists(dotConfigPath)) {
    return dotConfigPath;
  }

  return path.join(workspaceFolderPath, TOMBI_CONFIG_FILE_NAME);
}

export function ensureTombiSchemaConfigText(text: string): { changed: boolean; text: string } {
  if (text.includes(TOMBI_SCHEMA_URL)) {
    return { changed: false, text };
  }

  const separator = text.trim().length === 0 ? '' : text.endsWith('\n') ? '\n' : '\n\n';
  return {
    changed: true,
    text: `${text}${separator}${TOMBI_SCHEMA_CONFIG_BLOCK}`,
  };
}

export async function ensureTombiSchemaConfigFile(
  configPath: string,
): Promise<TombiSchemaInjectionResult> {
  let existingText = '';
  let existed = true;

  try {
    existingText = await fs.promises.readFile(configPath, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') {
      throw error;
    }
    existed = false;
  }

  const next = ensureTombiSchemaConfigText(existingText);
  if (!next.changed) {
    return 'already-configured';
  }

  await fs.promises.mkdir(path.dirname(configPath), { recursive: true });
  await fs.promises.writeFile(configPath, next.text, 'utf8');

  return existed ? 'updated' : 'created';
}
