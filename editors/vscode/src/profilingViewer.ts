import { randomUUID } from 'node:crypto';
import * as fs from 'node:fs';
import * as http from 'node:http';
import * as path from 'node:path';

import * as vscode from 'vscode';

import type { ProfileArtifacts } from './profilingTypes';
import { buildSpeedscopeUrl } from './profilingViewerUrl';

type ProfileEntry = {
  tracePath: string;
  title: string;
};

const corsHeaders = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, HEAD, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type',
};

const contentTypes: Record<string, string> = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.txt': 'text/plain; charset=utf-8',
  '.wasm': 'application/wasm',
  '.woff2': 'font/woff2',
};

export class SpeedscopeProfileViewer implements vscode.Disposable {
  private readonly profiles = new Map<string, ProfileEntry>();
  private server: http.Server | undefined;
  private port: number | undefined;

  constructor(private readonly context: vscode.ExtensionContext) {}

  async open(artifacts: ProfileArtifacts): Promise<vscode.Uri> {
    const port = await this.ensureServer();
    const id = randomUUID();
    const title = `Vizsla ${path.basename(artifacts.dir)}`;

    this.profiles.set(id, { tracePath: artifacts.trace, title });

    const viewerUri = vscode.Uri.parse(`http://127.0.0.1:${port}/index.html`);
    const profileUri = vscode.Uri.parse(`http://127.0.0.1:${port}/profiles/${id}/trace.json`);
    const externalViewerUri = await vscode.env.asExternalUri(viewerUri);
    const externalProfileUri = await vscode.env.asExternalUri(profileUri);
    const targetUrl = buildSpeedscopeUrl(
      externalViewerUri.toString(),
      externalProfileUri.toString(),
      title,
    );
    const targetUri = vscode.Uri.parse(targetUrl);

    await vscode.env.openExternal(targetUri);
    return targetUri;
  }

  dispose(): void {
    this.profiles.clear();
    this.port = undefined;
    this.server?.close();
    this.server = undefined;
  }

  private async ensureServer(): Promise<number> {
    if (this.port !== undefined) {
      return this.port;
    }

    const viewerRoot = this.viewerRoot();
    const indexPath = path.join(viewerRoot, 'index.html');
    if (!fs.existsSync(indexPath)) {
      throw new Error(`Speedscope assets not found at ${viewerRoot}`);
    }

    const server = http.createServer((request, response) => {
      void this.handleRequest(viewerRoot, request, response).catch((error: unknown) => {
        if (!response.headersSent) {
          response.writeHead(500, corsHeaders);
        }
        response.end((error as Error).message);
      });
    });

    await new Promise<void>((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', () => {
        server.off('error', reject);
        resolve();
      });
    });

    const address = server.address();
    if (typeof address !== 'object' || address === null) {
      server.close();
      throw new Error('Speedscope server did not bind to a TCP port');
    }

    this.server = server;
    this.port = address.port;
    return address.port;
  }

  private async handleRequest(
    viewerRoot: string,
    request: http.IncomingMessage,
    response: http.ServerResponse,
  ): Promise<void> {
    if (request.method === 'OPTIONS') {
      response.writeHead(204, corsHeaders);
      response.end();
      return;
    }

    if (request.method !== 'GET' && request.method !== 'HEAD') {
      response.writeHead(405, corsHeaders);
      response.end('Method not allowed');
      return;
    }

    const requestUrl = new URL(request.url ?? '/', 'http://127.0.0.1');
    if (requestUrl.pathname.startsWith('/profiles/')) {
      await this.serveProfile(requestUrl.pathname, request, response);
      return;
    }

    await this.serveAsset(viewerRoot, requestUrl.pathname, request, response);
  }

  private async serveProfile(
    pathname: string,
    request: http.IncomingMessage,
    response: http.ServerResponse,
  ): Promise<void> {
    const match = /^\/profiles\/([^/]+)\/trace\.json$/.exec(pathname);
    const profile = match ? this.profiles.get(match[1]) : undefined;
    if (!profile) {
      response.writeHead(404, corsHeaders);
      response.end('Profile not found');
      return;
    }

    const stat = await fs.promises.stat(profile.tracePath).catch(() => undefined);
    if (!stat?.isFile()) {
      response.writeHead(404, corsHeaders);
      response.end('Trace file not found');
      return;
    }

    response.writeHead(200, {
      ...corsHeaders,
      'Cache-Control': 'no-store',
      'Content-Length': stat.size,
      'Content-Type': 'application/json; charset=utf-8',
    });

    if (request.method === 'HEAD') {
      response.end();
      return;
    }

    await pipeFile(profile.tracePath, response);
  }

  private async serveAsset(
    viewerRoot: string,
    pathname: string,
    request: http.IncomingMessage,
    response: http.ServerResponse,
  ): Promise<void> {
    const assetPath = assetPathForRequest(viewerRoot, pathname);
    if (!assetPath) {
      response.writeHead(404, corsHeaders);
      response.end('Not found');
      return;
    }

    const stat = await fs.promises.stat(assetPath).catch(() => undefined);
    if (!stat?.isFile()) {
      response.writeHead(404, corsHeaders);
      response.end('Not found');
      return;
    }

    response.writeHead(200, {
      ...corsHeaders,
      'Cache-Control': 'public, max-age=3600',
      'Content-Length': stat.size,
      'Content-Type': contentTypes[path.extname(assetPath).toLowerCase()] ?? 'application/octet-stream',
    });

    if (request.method === 'HEAD') {
      response.end();
      return;
    }

    await pipeFile(assetPath, response);
  }

  private viewerRoot(): string {
    return this.context.asAbsolutePath(path.join('dist', 'speedscope'));
  }
}

function assetPathForRequest(viewerRoot: string, pathname: string): string | undefined {
  const relativePath = decodeURIComponent(pathname === '/' ? '/index.html' : pathname).replace(
    /^\/+/,
    '',
  );
  const candidate = path.resolve(viewerRoot, relativePath);
  const relativeFromRoot = path.relative(viewerRoot, candidate);
  if (relativeFromRoot.startsWith('..') || path.isAbsolute(relativeFromRoot)) {
    return undefined;
  }
  return candidate;
}

function pipeFile(filePath: string, response: http.ServerResponse): Promise<void> {
  return new Promise((resolve, reject) => {
    const stream = fs.createReadStream(filePath);
    stream.once('error', reject);
    stream.once('end', resolve);
    stream.pipe(response);
  });
}
