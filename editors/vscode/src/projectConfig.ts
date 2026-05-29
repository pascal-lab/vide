import * as path from 'node:path';

export * from './projectConfigCommon';

import { PROJECT_CONFIG_FILE_NAME } from './projectConfigCommon';

export function getProjectConfigPath(
  workspaceFolderPath: string,
  fileName = PROJECT_CONFIG_FILE_NAME,
): string {
  return path.join(workspaceFolderPath, fileName);
}
