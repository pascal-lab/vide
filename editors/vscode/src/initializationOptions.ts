type ConfigurationReader = {
  get<T>(section: string): T | undefined;
};

function setting<T>(config: ConfigurationReader, section: string, fallback: T): T {
  return config.get<T>(section) ?? fallback;
}

export function serverInitializationOptions(
  config: ConfigurationReader,
): Record<string, unknown> {
  return {
    files: {
      excludeDirs: setting(config, 'files.excludeDirs', []),
      watcher: setting(config, 'files.watcher', 'client'),
    },
    workspace: {
      auto: { reload: setting(config, 'workspace.auto.reload', true) },
    },
    scope: {
      visibility: setting(config, 'scope.visibility', 'private'),
    },
    formatter: {
      provider: setting(config, 'formatter.provider', 'verible'),
      path: setting<string | null>(config, 'formatter.path', null),
      args: setting(config, 'formatter.args', ['--failsafe_success=false']),
    },
    formatting: {
      on: { enter: setting(config, 'formatting.on.enter', true) },
      in: { comments: setting(config, 'formatting.in.comments', true) },
      indent: { width: setting(config, 'formatting.indent.width', 4) },
    },
    inlayHints: {
      port: {
        connection: { enable: setting(config, 'inlayHints.port.connection.enable', true) },
      },
      parameter: {
        assignment: { enable: setting(config, 'inlayHints.parameter.assignment.enable', true) },
      },
      end: {
        structure: { enable: setting(config, 'inlayHints.end.structure.enable', true) },
      },
    },
    lens: {
      instantiations: { enable: setting(config, 'lens.instantiations.enable', true) },
    },
    semantic: {
      tokens: {
        port: {
          clk: {
            rst: { enable: setting(config, 'semantic.tokens.port.clk.rst.enable', true) },
          },
          input: {
            output: {
              enable: setting(config, 'semantic.tokens.port.input.output.enable', true),
            },
          },
        },
      },
    },
    diagnostics: {
      enable: setting(config, 'diagnostics.enable', true),
      update: setting(config, 'diagnostics.update', 'onSave'),
      parse: { enable: setting(config, 'diagnostics.parse.enable', true) },
      semantic: { enable: setting(config, 'diagnostics.semantic.enable', true) },
      slang: {
        warnings: setting(config, 'diagnostics.slang.warnings', []),
        rules: setting(config, 'diagnostics.slang.rules', []),
      },
    },
    signature: {
      help: {
        params: { only: setting(config, 'signature.help.params.only', false) },
      },
    },
    qihe: {
      command: setting(config, 'qihe.command', 'qihe'),
      autoConfigureArgsFromManifest: setting(
        config,
        'qihe.autoConfigureArgsFromManifest',
        true,
      ),
      compileArgs: setting(config, 'qihe.compileArgs', []),
      runArgs: setting(config, 'qihe.runArgs', ['-g', 'std']),
    },
  };
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
