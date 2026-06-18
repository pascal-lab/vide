export type ConfigurationReader = {
  get<T>(section: string): T | undefined;
};

function defaultQiheCommand(platform: NodeJS.Platform): string {
  return platform === 'win32' ? 'qihe.bat' : 'qihe';
}

function configuredQiheCommand(config: ConfigurationReader): string | null {
  const command = config.get<string | null>('qihe.command');
  return typeof command === 'string' && command.trim().length > 0 ? command.trim() : null;
}

export function resolvedQiheCommand(
  config: ConfigurationReader,
  platform: NodeJS.Platform = process.platform,
): string {
  return configuredQiheCommand(config) ?? defaultQiheCommand(platform);
}
