import * as vscode from 'vscode';
import {
  LanguageClient,
  type ServerOptions,
} from 'vscode-languageclient/node';

import { createNodeClientOptions } from '../common/clientOptions';
import { createProvideExpandedRenameEdits } from '../common/renameMiddleware';
import {
  projectStatusNotification,
  reloadWorkspaceRequest,
} from '../videStatus';
import type { ServerStatus } from '../status';
import {
  createServerEnv,
  readConfiguration,
  resolveServerLaunch,
} from './serverLaunch';

type Logger = (message: string) => void;

export class NodeClientController {
  private client: LanguageClient | undefined;

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly options: {
      outputChannel: vscode.OutputChannel;
      log: Logger;
      updateServerStatus: (status: ServerStatus, detail?: string) => void;
      showLanguageServerErrorMessage: (message: string) => Promise<void>;
      handleProjectStatusNotification: (params: unknown) => void;
      registerQiheNotifications: (client: LanguageClient) => void;
    },
  ) {}

  getClient(): LanguageClient | undefined {
    return this.client;
  }

  hasClient(): boolean {
    return this.client !== undefined;
  }

  async start(): Promise<void> {
    try {
      this.options.updateServerStatus('starting');
      this.options.log('[INFO] Starting language server...');
      this.client = await this.createClient();
      this.registerProjectStatusNotifications(this.client);
      this.options.registerQiheNotifications(this.client);
      await this.client.start();
      this.options.log('[INFO] Language server started successfully');
      this.options.updateServerStatus('ready');
    } catch (error) {
      const message = (error as Error).message;
      this.client = undefined;
      this.options.log(`[ERROR] Failed to start language server: ${message}`);
      this.options.log(`[ERROR] ${(error as Error).stack}`);
      this.options.updateServerStatus('error', message);
      await this.options.showLanguageServerErrorMessage(
        vscode.l10n.t('Failed to start Vide Language Server: {0}', message),
      );
    }
  }

  async stop(): Promise<void> {
    if (!this.client) {
      this.options.updateServerStatus('stopped');
      return;
    }

    this.options.updateServerStatus('stopping');
    this.options.log('[INFO] Stopping language server...');
    try {
      await this.client.stop();
      this.options.log('[INFO] Language server stopped');
    } catch (error) {
      this.options.log(`[ERROR] Error stopping language server: ${(error as Error).message}`);
    } finally {
      this.client = undefined;
      this.options.updateServerStatus('stopped');
    }
  }

  async restart(): Promise<void> {
    this.options.log('[INFO] Restarting language server...');
    await this.stop();
    await this.start();
  }

  async reloadWorkspace(): Promise<void> {
    if (!this.client) {
      await this.options.showLanguageServerErrorMessage(
        vscode.l10n.t('Vide language server is not running.'),
      );
      return;
    }

    try {
      await this.client.sendRequest('workspace/executeCommand', {
        command: reloadWorkspaceRequest,
        arguments: [],
      });
    } catch (error) {
      const message = vscode.l10n.t(
        'Failed to reload Vide project configuration: {0}',
        (error as Error).message,
      );
      this.options.log(`[ERROR] ${message}`);
      await this.options.showLanguageServerErrorMessage(message);
    }
  }

  private async createClient(): Promise<LanguageClient> {
    this.options.log('[INFO] Creating language client...');

    const config = readConfiguration();
    const launch = resolveServerLaunch(this.context, config, this.options.log);
    const serverArgs = [...launch.args, ...launch.additionalArgs];

    const commonEnv = {
      ...createServerEnv(),
    };

    const serverOptions: ServerOptions = {
      run: {
        command: launch.command,
        args: serverArgs,
        options: { cwd: launch.cwd, env: commonEnv },
      },
      debug: {
        command: launch.command,
        args: serverArgs,
        options: {
          cwd: launch.cwd,
          env: createServerEnv('debug', 'full'),
        },
      },
    };

    const clientOptions = createNodeClientOptions({
      outputChannel: this.options.outputChannel,
      trace: config.trace,
      provideRenameEdits: createProvideExpandedRenameEdits(
        () => this.client,
        (message) => this.options.log(`[WARN] ${message}`),
      ),
    });

    this.options.log('[INFO] Creating LanguageClient instance...');
    return new LanguageClient(
      'vide',
      vscode.l10n.t('Vide Language Server'),
      serverOptions,
      clientOptions,
    );
  }

  private registerProjectStatusNotifications(languageClient: LanguageClient): void {
    languageClient.onNotification(projectStatusNotification, (params: unknown) => {
      this.options.handleProjectStatusNotification(params);
    });
  }
}
