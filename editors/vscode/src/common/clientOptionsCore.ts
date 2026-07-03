import type { LanguageClientOptions } from 'vscode-languageclient';

import {
  serverInitializationOptions,
  type ConfigurationReader,
} from './initializationOptions';

type ClientMiddleware = NonNullable<LanguageClientOptions['middleware']>;

export type VideClientOptionsCoreParams = {
  documentSelector: NonNullable<LanguageClientOptions['documentSelector']>;
  configuration: ConfigurationReader;
  provideRenameEdits: NonNullable<ClientMiddleware['provideRenameEdits']>;
};

export type VideClientOptionsCore = {
  documentSelector: NonNullable<LanguageClientOptions['documentSelector']>;
  initializationOptions: Record<string, unknown>;
  middleware: Pick<ClientMiddleware, 'provideRenameEdits'>;
};

export function createVideClientOptionsCore({
  documentSelector,
  configuration,
  provideRenameEdits,
}: VideClientOptionsCoreParams): VideClientOptionsCore {
  return {
    documentSelector,
    initializationOptions: serverInitializationOptions(configuration),
    middleware: {
      provideRenameEdits,
    },
  };
}
