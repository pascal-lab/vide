import { USER_CONFIG_SETTINGS } from './generated/configuration';

type ConfigurationReader = {
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

export function serverInitializationOptions(
  config: ConfigurationReader,
  platform: NodeJS.Platform = process.platform,
): Record<string, unknown> {
  const options: Record<string, unknown> = {};

  for (const configSetting of USER_CONFIG_SETTINGS) {
    const value =
      configSetting.vscodeSection === 'qihe.command'
        ? qiheCommandSetting(
            config,
            configSetting.vscodeSection,
            configSetting.defaultValue,
            platform,
          )
        : setting(config, configSetting.vscodeSection, configSetting.defaultValue);

    assignNestedValue(options, configSetting.path, value);
  }

  return options;
}

export function diagnosticsProfilingInitializationOptions(
  config: ConfigurationReader,
): Record<string, unknown> {
  const options = serverInitializationOptions(config);

  return {
    ...options,
    files: {
      ...(options.files as Record<string, unknown>),
      watcher: 'server',
    },
  };
}

function assignNestedValue(
  target: Record<string, unknown>,
  path: readonly string[],
  value: unknown,
): void {
  let cursor = target;

  for (const key of path.slice(0, -1)) {
    const existing = cursor[key];
    if (typeof existing === 'object' && existing !== null && !Array.isArray(existing)) {
      cursor = existing as Record<string, unknown>;
    } else {
      const next: Record<string, unknown> = {};
      cursor[key] = next;
      cursor = next;
    }
  }

  const leaf = path.at(-1);
  if (leaf) {
    cursor[leaf] = value;
  }
}
