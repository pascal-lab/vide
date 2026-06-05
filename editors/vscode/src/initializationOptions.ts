import { USER_CONFIG_SETTINGS } from './generated/configuration';
import { resolvedQiheCommand, type ConfigurationReader } from './qiheCommand';

function setting<T>(config: ConfigurationReader, section: string, fallback: T): T {
  return config.get<T>(section) ?? fallback;
}

export function serverInitializationOptions(
  config: ConfigurationReader,
  platform: NodeJS.Platform = process.platform,
): Record<string, unknown> {
  const options: Record<string, unknown> = {};

  for (const configSetting of USER_CONFIG_SETTINGS) {
    const value =
      configSetting.vscodeSection === 'qihe.command'
        ? resolvedQiheCommand(config, platform)
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
