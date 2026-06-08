import { spawnSync } from 'node:child_process';

export function run(
  command: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv = process.env,
): void {
  const result = spawnSync(command, args, {
    cwd,
    env,
    shell: false,
    stdio: 'inherit',
  });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
  }
}

export function optionalEnv(name: string): string | undefined {
  const value = process.env[name]?.trim();
  return value ? value : undefined;
}

export function sanitizedVsceEnv(): NodeJS.ProcessEnv {
  const env = { ...process.env };

  for (const key of Object.keys(env)) {
    const normalized = key.toLowerCase();
    if (
      normalized === 'npm_config_verify_deps_before_run' ||
      normalized === 'npm_config_npm_globalconfig' ||
      normalized === 'npm_config__jsr_registry'
    ) {
      delete env[key];
    }
  }

  return env;
}
