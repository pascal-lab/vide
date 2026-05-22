export type ServerStatus = 'starting' | 'ready' | 'stopping' | 'stopped' | 'error';

export interface LanguageStatusPresentation {
  text: string;
  detail: string;
  severity: 'information' | 'warning' | 'error';
  busy: boolean;
}

export type ServerStatusPresentation = LanguageStatusPresentation;

export interface ServerStatusMessages {
  text: string;
  startingDetail: string;
  readyDetail: string;
  stoppingDetail: string;
  stoppedDetail: string;
  errorDetail: string;
}

export const defaultServerStatusMessages: ServerStatusMessages = {
  text: 'Vizsla',
  startingDetail: 'Vizsla language server is starting.',
  readyDetail: 'Vizsla language server is running.',
  stoppingDetail: 'Vizsla language server is stopping.',
  stoppedDetail: 'Vizsla language server is stopped.',
  errorDetail: 'Vizsla language server failed.',
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
        text: messages.text,
        detail: `${messages.startingDetail}${suffix}`,
        severity: 'information',
        busy: true,
      };
    case 'ready':
      return {
        text: messages.text,
        detail: `${messages.readyDetail}${suffix}`,
        severity: 'information',
        busy: false,
      };
    case 'stopping':
      return {
        text: messages.text,
        detail: `${messages.stoppingDetail}${suffix}`,
        severity: 'information',
        busy: true,
      };
    case 'stopped':
      return {
        text: messages.text,
        detail: `${messages.stoppedDetail}${suffix}`,
        severity: 'information',
        busy: false,
      };
    case 'error':
      return {
        text: messages.text,
        detail: `${messages.errorDetail}${suffix}`,
        severity: 'error',
        busy: false,
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

export function initialProjectStatus(): ProjectStatus {
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

export type ProjectStatusPresentation = LanguageStatusPresentation;

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

export interface VizslaStatusInput {
  serverStatus: ServerStatus;
  serverDetail?: string;
  projectStatus: ProjectStatus;
}

export type VizslaStatusPhase =
  | {
      kind: 'server';
      status: Exclude<ServerStatus, 'ready'>;
      detail?: string;
    }
  | {
      kind: 'project';
      status: ProjectStatus;
    };

export interface VizslaStatusMessages {
  server: ServerStatusMessages;
  project: ProjectStatusMessages;
}

export function selectVizslaStatusPhase(status: VizslaStatusInput): VizslaStatusPhase {
  if (status.serverStatus === 'ready') {
    return {
      kind: 'project',
      status: status.projectStatus,
    };
  }

  if (status.serverDetail === undefined) {
    return {
      kind: 'server',
      status: status.serverStatus,
    };
  }

  return {
    kind: 'server',
    status: status.serverStatus,
    detail: status.serverDetail,
  };
}

export function getVizslaStatusPresentation(
  status: VizslaStatusInput,
  messages: VizslaStatusMessages,
): LanguageStatusPresentation {
  const phase = selectVizslaStatusPhase(status);

  switch (phase.kind) {
    case 'server':
      return getServerStatusPresentation(phase.status, phase.detail, messages.server);
    case 'project':
      return getProjectStatusPresentation(phase.status, messages.project);
  }
}

function asStringArray(value: unknown): string[] | undefined {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
    ? value
    : undefined;
}
