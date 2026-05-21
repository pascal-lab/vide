export type ServerStatus = 'starting' | 'ready' | 'stopping' | 'stopped' | 'error';

export interface ServerStatusPresentation {
  text: string;
  tooltip: string;
  color?: string;
  backgroundColor?: string;
}

export interface ServerStatusMessages {
  startingText: string;
  startingTooltip: string;
  readyText: string;
  readyTooltip: string;
  stoppingText: string;
  stoppingTooltip: string;
  stoppedText: string;
  stoppedTooltip: string;
  errorText: string;
  errorTooltip: string;
}

export const defaultServerStatusMessages: ServerStatusMessages = {
  startingText: '$(sync~spin) Vizsla Starting',
  startingTooltip: 'Vizsla language server is starting.',
  readyText: '$(check) Vizsla Ready',
  readyTooltip: 'Vizsla language server is running.',
  stoppingText: '$(debug-stop) Vizsla Stopping',
  stoppingTooltip: 'Vizsla language server is stopping.',
  stoppedText: '$(circle-slash) Vizsla Stopped',
  stoppedTooltip: 'Vizsla language server is stopped.',
  errorText: '$(error) Vizsla Error',
  errorTooltip: 'Vizsla language server failed.',
};

export function getServerStatusPresentation(
  status: ServerStatus,
  detail?: string,
  messages: ServerStatusMessages = defaultServerStatusMessages,
): ServerStatusPresentation {
  const suffix = detail ? `\n${detail}` : '';

  switch (status) {
    case 'starting':
      return {
        text: messages.startingText,
        tooltip: `${messages.startingTooltip}${suffix}`,
      };
    case 'ready':
      return {
        text: messages.readyText,
        tooltip: `${messages.readyTooltip}${suffix}`,
      };
    case 'stopping':
      return {
        text: messages.stoppingText,
        tooltip: `${messages.stoppingTooltip}${suffix}`,
      };
    case 'stopped':
      return {
        text: messages.stoppedText,
        tooltip: `${messages.stoppedTooltip}${suffix}`,
      };
    case 'error':
      return {
        text: messages.errorText,
        tooltip: `${messages.errorTooltip}${suffix}`,
        backgroundColor: 'statusBarItem.errorBackground',
      };
  }
}
