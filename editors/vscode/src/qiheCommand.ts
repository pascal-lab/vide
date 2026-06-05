import { USER_CONFIG_SETTINGS } from './generated/configuration';

export type ConfigurationReader = {
  get<T>(section: string): T | undefined;
  inspect?<T>(section: string): ConfigurationInspection<T> | undefined;
};

type ConfigurationInspection<T> = {
  defaultValue?: T;
  globalValue?: T;
  workspaceValue?: T;
  workspaceFolderValue?: T;
  defaultLanguageValue?: T;
  globalLanguageValue?: T;
  workspaceLanguageValue?: T;
  workspaceFolderLanguageValue?: T;
};

function setting<T>(config: ConfigurationReader, section: string, fallback: T): T {
  return config.get<T>(section) ?? fallback;
}

function defaultQiheCommand(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'qihe.bat' : 'qihe';
}

function hasConfiguredValue<T>(inspection: ConfigurationInspection<T> | undefined): boolean {
  return (
    inspection?.globalValue !== undefined ||
    inspection?.workspaceValue !== undefined ||
    inspection?.workspaceFolderValue !== undefined ||
    inspection?.globalLanguageValue !== undefined ||
    inspection?.workspaceLanguageValue !== undefined ||
    inspection?.workspaceFolderLanguageValue !== undefined
  );
}

function qiheCommandSetting(
  config: ConfigurationReader,
  section: string,
  fallback: unknown,
  platform: NodeJS.Platform,
): string {
  const command = setting(config, section, fallback);
  if (typeof command !== 'string') {
    return defaultQiheCommand(platform);
  }

  if (hasConfiguredValue(config.inspect?.<string>(section))) {
    return command;
  }

  return command === fallback ? defaultQiheCommand(platform) : command;
}

export function resolvedQiheCommand(
  config: ConfigurationReader,
  platform: NodeJS.Platform = process.platform,
): string {
  const qiheCommandConfig = USER_CONFIG_SETTINGS.find(
    (configSetting) => configSetting.vscodeSection === 'qihe.command',
  );
  return qiheCommandSetting(
    config,
    'qihe.command',
    qiheCommandConfig?.defaultValue ?? 'qihe',
    platform,
  );
}
