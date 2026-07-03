import type * as vscode from "vscode";

import { USER_CONFIG_SETTINGS } from "../generated/configuration";

const browserRestartConfigurationKeys = USER_CONFIG_SETTINGS.map(
  (setting) => setting.vscodeKey,
);

export function affectsBrowserClientConfiguration(
  event: Pick<vscode.ConfigurationChangeEvent, "affectsConfiguration">,
): boolean {
  return browserRestartConfigurationKeys.some((key) => event.affectsConfiguration(key));
}
