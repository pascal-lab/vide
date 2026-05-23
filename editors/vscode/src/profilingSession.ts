import { type ChildProcessWithoutNullStreams } from 'node:child_process';

import * as vscode from 'vscode';

import type { LspMessage, ProfileTarget } from './profilingTypes';

export class LspProfileSession {
  private buffer = Buffer.alloc(0);
  private nextId = 1;
  private disposed = false;
  private readonly pending = new Map<
    number,
    {
      method: string;
      resolve: (message: LspMessage) => void;
      reject: (error: Error) => void;
      timer: NodeJS.Timeout;
    }
  >();

  constructor(
    private readonly child: ChildProcessWithoutNullStreams,
    private readonly channel: vscode.OutputChannel,
    private readonly configurationProvider: () => Record<string, unknown>,
  ) {
    child.stdout.on('data', (chunk: Buffer) => this.readStdout(chunk));
    child.stderr.on('data', (chunk: Buffer) => {
      const text = chunk.toString('utf8').trimEnd();
      if (text.length > 0) {
        this.channel.appendLine(text);
      }
    });
    child.on('error', (error) => this.failPending(error));
    child.on('exit', (code, signal) => {
      if (this.pending.size > 0) {
        this.failPending(new Error(`profile server exited with code ${code} signal ${signal}`));
      }
    });
  }

  async initialize(target: ProfileTarget, timeoutMs: number): Promise<void> {
    await this.request(
      'initialize',
      {
        processId: process.pid,
        rootUri: vscode.Uri.file(target.workspaceRoot).toString(),
        workspaceFolders: [
          {
            uri: vscode.Uri.file(target.workspaceRoot).toString(),
            name: target.workspaceName,
          },
        ],
        capabilities: {
          window: { workDoneProgress: true },
          workspace: {
            workspaceFolders: true,
            configuration: true,
            diagnostics: { refreshSupport: true },
          },
          textDocument: {
            synchronization: { didSave: true },
            diagnostic: {
              dynamicRegistration: false,
              relatedDocumentSupport: true,
            },
          },
        },
        initializationOptions: this.configurationProvider(),
        clientInfo: { name: 'vizsla-profile-runner', version: 'local' },
      },
      timeoutMs,
    );
    this.notify('initialized', {});
    if (target.scope === 'document') {
      this.notify('textDocument/didOpen', {
        textDocument: {
          uri: target.document.uri.toString(),
          languageId: target.document.languageId,
          version: target.document.version,
          text: target.document.getText(),
        },
      });
    }
  }

  request(method: string, params: unknown, timeoutMs: number): Promise<LspMessage> {
    const id = this.nextId;
    this.nextId += 1;
    const message = { jsonrpc: '2.0', id, method, params };
    this.send(message);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} timed out after ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { method, resolve, reject, timer });
    });
  }

  notify(method: string, params: unknown): void {
    this.send({ jsonrpc: '2.0', method, params });
  }

  async waitForExit(timeoutMs: number): Promise<void> {
    if (this.child.exitCode !== null || this.child.signalCode !== null) {
      return;
    }

    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.dispose();
        reject(new Error(`profile server did not exit after ${timeoutMs} ms`));
      }, timeoutMs);
      this.child.once('exit', () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }

  dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(new Error(`${pending.method} cancelled`));
      this.pending.delete(id);
    }
    if (this.child.exitCode === null && this.child.signalCode === null) {
      this.child.kill();
    }
  }

  private readStdout(chunk: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    for (;;) {
      const headerEnd = this.buffer.indexOf('\r\n\r\n');
      if (headerEnd < 0) {
        return;
      }

      const header = this.buffer.subarray(0, headerEnd).toString('ascii');
      const contentLength = /Content-Length:\s*(\d+)/i.exec(header)?.[1];
      if (!contentLength) {
        this.buffer = this.buffer.subarray(headerEnd + 4);
        continue;
      }

      const length = Number(contentLength);
      const messageStart = headerEnd + 4;
      const messageEnd = messageStart + length;
      if (this.buffer.length < messageEnd) {
        return;
      }

      const text = this.buffer.subarray(messageStart, messageEnd).toString('utf8');
      this.buffer = this.buffer.subarray(messageEnd);
      try {
        this.handleMessage(JSON.parse(text) as LspMessage);
      } catch (error) {
        this.channel.appendLine(
          `[WARN] Failed to parse server message: ${(error as Error).message}`,
        );
      }
    }
  }

  private handleMessage(message: LspMessage): void {
    const id = typeof message.id === 'number' ? message.id : undefined;
    const method = typeof message.method === 'string' ? message.method : undefined;

    if (id !== undefined && method === undefined) {
      const pending = this.pending.get(id);
      if (!pending) {
        return;
      }
      this.pending.delete(id);
      clearTimeout(pending.timer);
      if (message.error) {
        pending.reject(new Error(JSON.stringify(message.error)));
      } else {
        pending.resolve(message);
      }
      return;
    }

    if (id !== undefined && method) {
      this.respondToServerRequest(id, method);
    }
  }

  private respondToServerRequest(id: number, method: string): void {
    const result = method === 'workspace/configuration' ? [this.configurationProvider()] : null;
    this.send({ jsonrpc: '2.0', id, result });
  }

  private send(message: unknown): void {
    const text = JSON.stringify(message);
    this.child.stdin.write(`Content-Length: ${Buffer.byteLength(text, 'utf8')}\r\n\r\n${text}`);
  }

  private failPending(error: Error): void {
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(error);
      this.pending.delete(id);
    }
  }
}
