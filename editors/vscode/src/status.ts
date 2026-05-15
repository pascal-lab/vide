export type ServerStatus = 'starting' | 'ready' | 'stopping' | 'stopped' | 'error';

export interface ServerStatusPresentation {
  text: string;
  tooltip: string;
  color?: string;
  backgroundColor?: string;
}

export function getServerStatusPresentation(
  status: ServerStatus,
  detail?: string,
): ServerStatusPresentation {
  const suffix = detail ? `\n${detail}` : '';

  switch (status) {
    case 'starting':
      return {
        text: '$(sync~spin) Vizsla Starting',
        tooltip: `Vizsla language server is starting.${suffix}`,
      };
    case 'ready':
      return {
        text: '$(check) Vizsla Ready',
        tooltip: `Vizsla language server is running.${suffix}`,
      };
    case 'stopping':
      return {
        text: '$(debug-stop) Vizsla Stopping',
        tooltip: `Vizsla language server is stopping.${suffix}`,
      };
    case 'stopped':
      return {
        text: '$(circle-slash) Vizsla Stopped',
        tooltip: `Vizsla language server is stopped.${suffix}`,
      };
    case 'error':
      return {
        text: '$(error) Vizsla Error',
        tooltip: `Vizsla language server failed.${suffix}`,
        backgroundColor: 'statusBarItem.errorBackground',
      };
  }
}
