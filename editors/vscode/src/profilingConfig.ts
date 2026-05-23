type ConfigurationReader = {
  get<T>(section: string): T | undefined;
};

export function diagnosticsProfilingInitializationOptions(
  config: ConfigurationReader,
): Record<string, unknown> {
  return {
    files_excludeDirs: config.get('files.excludeDirs') ?? [],
    // The profiling runner does not implement client-side file watching.
    files_watcher: 'server',
    diagnostics: {
      enable: config.get('diagnostics.enable') ?? true,
      parse: { enable: config.get('diagnostics.parse.enable') ?? true },
      semantic: { enable: config.get('diagnostics.semantic.enable') ?? true },
      slang: {
        warnings: config.get('diagnostics.slang.warnings') ?? [],
        rules: config.get('diagnostics.slang.rules') ?? [],
      },
    },
  };
}
