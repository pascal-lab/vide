export function stripProfileArgs(args: string[]): string[] {
  const stripped: string[] = [];
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (['--log', '--log_file', '--profile_trace'].includes(arg)) {
      index += 1;
      continue;
    }
    if (
      arg.startsWith('--log=') ||
      arg.startsWith('--log_file=') ||
      arg.startsWith('--profile_trace=')
    ) {
      continue;
    }
    stripped.push(arg);
  }
  return stripped;
}
