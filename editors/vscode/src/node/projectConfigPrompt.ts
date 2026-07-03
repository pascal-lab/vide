import * as fs from 'node:fs';
import * as path from 'node:path';

import * as vscode from 'vscode';

import {
  DEFAULT_PROJECT_CONFIG_TEXT,
  PROJECT_CONFIG_FILE_NAMES,
  PROJECT_CONFIG_FILE_NAME,
  PROJECT_SOURCE_FILE_GLOB,
  getProjectConfigPath,
} from '../projectConfig';

type Logger = (message: string) => void;

export type ProjectConfigPromptActions = {
  hasClient: () => boolean;
  restartClient: (context: vscode.ExtensionContext) => Promise<void>;
  log: Logger;
};

type ProjectConfigTarget = {
  folderName: string;
  configPath: string;
};

export async function promptForMissingProjectConfigs(
  context: vscode.ExtensionContext,
  actions: ProjectConfigPromptActions,
): Promise<void> {
  const targets = await findMissingProjectConfigTargets(actions.log);

  if (targets.length === 0) {
    return;
  }

  const createConfigAction =
    targets.length === 1
      ? vscode.l10n.t('Create Manifest')
      : vscode.l10n.t('Create Manifests');
  const restartNotice = vscode.l10n.t(
    'Creating a manifest will restart the Vide language server so the workspace can reload it.',
  );
  const promptMessage =
    targets.length === 1
      ? vscode.l10n.t(
          'No Vide project manifest was detected in {0}. Project-aware features like semantic diagnostics, navigation, and references may be severely limited. {1}',
          targets[0].folderName,
          restartNotice,
        )
      : vscode.l10n.t(
          'No Vide project manifest was detected in {0} workspace folders. Project-aware features like semantic diagnostics, navigation, and references may be severely limited. {1}',
          targets.length,
          restartNotice,
        );

  const selection = await vscode.window.showWarningMessage(promptMessage, createConfigAction);
  if (selection !== createConfigAction) {
    return;
  }

  await createProjectConfigs(context, targets, actions);
}

export async function createProjectConfigsFromRootUris(
  context: vscode.ExtensionContext,
  rootUris: readonly string[],
  actions: ProjectConfigPromptActions,
): Promise<void> {
  await createProjectConfigs(context, projectConfigTargetsFromRootUris(rootUris), actions);
}

async function findMissingProjectConfigTargets(log: Logger): Promise<ProjectConfigTarget[]> {
  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  const targets: ProjectConfigTarget[] = [];

  for (const folder of workspaceFolders) {
    if (folder.uri.scheme !== 'file') {
      log(
        `[WARN] Skipping project config creation for non-file workspace: ${folder.uri.toString()}`,
      );
      continue;
    }

    const existingConfigPath = PROJECT_CONFIG_FILE_NAMES
      .map((fileName) => getProjectConfigPath(folder.uri.fsPath, fileName))
      .find((configPath) => fs.existsSync(configPath));
    if (existingConfigPath) {
      log(`[INFO] Found project config: ${existingConfigPath}`);
      continue;
    }

    const sourceFiles = await vscode.workspace.findFiles(
      new vscode.RelativePattern(folder, PROJECT_SOURCE_FILE_GLOB),
      undefined,
      1,
    );
    if (sourceFiles.length === 0) {
      log(
        `[INFO] Skipping project config prompt for workspace without Verilog/SystemVerilog files: ${folder.name}`,
      );
      continue;
    }

    const configPath = getProjectConfigPath(folder.uri.fsPath);
    targets.push({ folderName: folder.name, configPath });
  }

  return targets;
}

function projectConfigTargetsFromRootUris(rootUris: readonly string[]): ProjectConfigTarget[] {
  return rootUris.map((rootUri) => {
    const uri = vscode.Uri.parse(rootUri);
    return {
      folderName: path.basename(uri.fsPath),
      configPath: getProjectConfigPath(uri.fsPath),
    };
  });
}

async function createProjectConfigs(
  context: vscode.ExtensionContext,
  targets: readonly ProjectConfigTarget[],
  actions: ProjectConfigPromptActions,
): Promise<void> {
  if (targets.length === 0) {
    return;
  }

  const createdConfigs: vscode.Uri[] = [];

  for (const { folderName, configPath } of targets) {
    try {
      await fs.promises.writeFile(configPath, DEFAULT_PROJECT_CONFIG_TEXT, {
        encoding: 'utf8',
        flag: 'wx',
      });
      createdConfigs.push(vscode.Uri.file(configPath));
      actions.log(`[INFO] Created default project config: ${configPath}`);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'EEXIST') {
        actions.log(`[INFO] Project config already exists: ${configPath}`);
        continue;
      }

      const errorMessage = vscode.l10n.t(
        'Failed to create {0} in {1}: {2}',
        PROJECT_CONFIG_FILE_NAME,
        folderName,
        (error as Error).message,
      );
      actions.log(`[WARN] ${errorMessage}`);
      void vscode.window.showWarningMessage(errorMessage);
    }
  }

  if (createdConfigs.length === 0) {
    return;
  }

  if (actions.hasClient()) {
    await actions.restartClient(context);
  }

  const createdMessage =
    createdConfigs.length === 1
      ? vscode.l10n.t('Created {0}.', PROJECT_CONFIG_FILE_NAME)
      : vscode.l10n.t(
          'Created {0} in {1} workspace folders.',
          PROJECT_CONFIG_FILE_NAME,
          createdConfigs.length,
        );
  const openConfigAction =
    createdConfigs.length === 1
      ? vscode.l10n.t('Open Manifest')
      : vscode.l10n.t('Open First Manifest');

  void vscode.window.showInformationMessage(createdMessage, openConfigAction).then(async (selection) => {
    if (selection !== openConfigAction) {
      return;
    }

    try {
      await vscode.window.showTextDocument(createdConfigs[0]);
    } catch (error) {
      actions.log(`[WARN] Failed to open ${PROJECT_CONFIG_FILE_NAME}: ${(error as Error).message}`);
    }
  });
}
