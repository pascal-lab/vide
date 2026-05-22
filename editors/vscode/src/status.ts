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
  startingText: '$(loading~spin) Vizsla',
  startingTooltip: 'Vizsla language server is starting.',
  readyText: 'Vizsla',
  readyTooltip: 'Vizsla language server is running.',
  stoppingText: '$(loading~spin) Vizsla',
  stoppingTooltip: 'Vizsla language server is stopping.',
  stoppedText: '$(circle-slash) Vizsla',
  stoppedTooltip: 'Vizsla language server is stopped.',
  errorText: '$(error) Vizsla',
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

export type ProjectStatusState = 'loading' | 'loaded' | 'none' | 'error';

export interface ProjectStatus {
  state: ProjectStatusState;
  manifestUris: string[];
  unconfiguredRootUris: string[];
  workspaceCount: number;
  errors: string[];
  message?: string;
}

export function projectStatusFallback(): ProjectStatus {
  return {
    state: 'loading',
    manifestUris: [],
    unconfiguredRootUris: [],
    workspaceCount: 0,
    errors: [],
  };
}

export function asProjectStatus(value: unknown): ProjectStatus | undefined {
  if (!value || typeof value !== 'object') {
    return undefined;
  }

  const params = value as Record<string, unknown>;
  const state = params.state;
  if (
    state !== 'loading' &&
    state !== 'loaded' &&
    state !== 'none' &&
    state !== 'error'
  ) {
    return undefined;
  }

  const manifestUris = asStringArray(params.manifestUris);
  const unconfiguredRootUris = asStringArray(params.unconfiguredRootUris);
  const errors = asStringArray(params.errors);
  const workspaceCount = params.workspaceCount;
  const message = params.message;
  if (
    !manifestUris ||
    !unconfiguredRootUris ||
    !errors ||
    typeof workspaceCount !== 'number' ||
    (message !== undefined && typeof message !== 'string')
  ) {
    return undefined;
  }

  return {
    state,
    manifestUris,
    unconfiguredRootUris,
    workspaceCount,
    errors,
    message,
  };
}

export interface ProjectStatusPresentation {
  text: string;
  detail: string;
  severity: 'information' | 'warning' | 'error';
  busy: boolean;
}

export interface ProjectStatusMessages {
  text: string;
  loadingDetail: string;
  loadedOneManifestDetail: string;
  loadedManyManifestsDetail: (count: number) => string;
  noManifestDetail: string;
  errorDetail: string;
}

export const defaultProjectStatusMessages: ProjectStatusMessages = {
  text: 'Vizsla',
  loadingDetail: 'Loading project configuration',
  loadedOneManifestDetail: 'Project manifest loaded',
  loadedManyManifestsDetail: (count) => `${count} project manifests loaded`,
  noManifestDetail: 'No project manifest',
  errorDetail: 'Project configuration failed',
};

export function getProjectStatusPresentation(
  status: ProjectStatus,
  messages: ProjectStatusMessages = defaultProjectStatusMessages,
): ProjectStatusPresentation {
  switch (status.state) {
    case 'loading':
      return {
        text: messages.text,
        detail: messages.loadingDetail,
        severity: 'information',
        busy: true,
      };
    case 'loaded':
      return {
        text: messages.text,
        detail:
          status.manifestUris.length === 1
            ? messages.loadedOneManifestDetail
            : messages.loadedManyManifestsDetail(status.manifestUris.length),
        severity: 'information',
        busy: false,
      };
    case 'none':
      return {
        text: messages.text,
        detail: messages.noManifestDetail,
        severity: 'warning',
        busy: false,
      };
    case 'error':
      return {
        text: messages.text,
        detail: messages.errorDetail,
        severity: 'error',
        busy: false,
      };
  }
}

function asStringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;
}
