const vscode = require('vscode');
const path = require('path');
const fs = require('fs');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

/** @type {LanguageClient | undefined} */
let client;

/** @type {vscode.OutputChannel} */
let outputChannel;

/**
 * Get the server executable path
 * @param {vscode.ExtensionContext} context
 * @returns {string | undefined}
 */
function getServerPath(context) {
  const platform = process.platform;
  const arch = process.arch;
  const binaryName = platform === 'win32' ? 'vizsla.exe' : 'vizsla';

  // Map platform-arch to folder names
  const supported_platform = new Set([
    'darwin-arm64',
    'darwin-x64',
    'linux-x64',
    'linux-arm64',
    'win32-x64',
  ]);

  const platformFolder = `${platform}-${arch}`;
  if (!supported_platform.has(platformFolder)) {
    outputChannel.appendLine(`[ERROR] Unsupported platform-architecture combination: ${platformFolder}`);
    return undefined;
  }

  // First, try to find bundled server
  const bundledPath = path.join(context.extensionPath, 'server', platformFolder, binaryName);
  outputChannel.appendLine(`[INFO] Looking for bundled server at: ${bundledPath}`);

  if (fs.existsSync(bundledPath)) {
    // Check if executable (Unix-like systems)
    if (platform !== 'win32') {
      try {
        fs.accessSync(bundledPath, fs.constants.X_OK);
        outputChannel.appendLine(`[INFO] Bundled server binary is executable`);
        return bundledPath;
      } catch (err) {
        outputChannel.appendLine(`[WARN] Bundled server binary exists but is not executable, attempting to fix...`);
        try {
          fs.chmodSync(bundledPath, 0o755);
          outputChannel.appendLine(`[INFO] Made bundled server binary executable`);
          return bundledPath;
        } catch (chmodErr) {
          outputChannel.appendLine(`[ERROR] Failed to make bundled binary executable: ${chmodErr.message}`);
        }
      }
    } else {
      outputChannel.appendLine(`[INFO] Found bundled server binary`);
      return bundledPath;
    }
  } else {
    outputChannel.appendLine(`[INFO] Bundled server binary not found at: ${bundledPath}`);
  }

  // If bundled server not found, try to find in PATH
  outputChannel.appendLine(`[INFO] Looking for ${binaryName} in system PATH...`);

  const { execSync } = require('child_process');
  try {
    const whichCommand = platform === 'win32' ? 'where' : 'which';
    const pathResult = execSync(`${whichCommand} ${binaryName}`, { encoding: 'utf8' }).trim();
    if (pathResult) {
      outputChannel.appendLine(`[INFO] Found ${binaryName} in PATH: ${pathResult}`);
      return pathResult.split('\n')[0]; // Return first match
    }
  } catch (err) {
    outputChannel.appendLine(`[INFO] ${binaryName} not found in PATH: ${err.message}`);
  }

  return undefined;
}

/**
 * Read configuration from VS Code settings
 * @returns {{command: string | undefined, args: string[], additionalArgs: string[], cwd: string | undefined, trace: string}}
 */
function readConfiguration() {
  const config = vscode.workspace.getConfiguration('vizslaLsp');
  const command = config.get('server.command');
  const args = config.get('server.args') ?? [];
  const additionalArgs = config.get('server.additionalArgs') ?? [];
  const cwd = config.get('server.cwd');
  const trace = config.get('trace.server') ?? 'off';

  if (!Array.isArray(args) || !Array.isArray(additionalArgs)) {
    vscode.window.showErrorMessage('vizslaLsp server arguments settings must be arrays of strings.');
    return {
      command: undefined,
      args: [],
      additionalArgs: [],
      cwd: undefined,
      trace
    };
  }

  return {
    command: typeof command === 'string' && command.length > 0 ? command : undefined,
    args,
    additionalArgs,
    cwd: typeof cwd === 'string' && cwd.length > 0 ? cwd : undefined,
    trace
  };
}

/**
 * Resolve working directory for the server
 * @param {vscode.ExtensionContext} context
 * @param {string | undefined} configuredCwd
 * @returns {string}
 */
function resolveWorkingDirectory(context, configuredCwd) {
  if (configuredCwd) {
    outputChannel.appendLine(`[INFO] Using configured working directory: ${configuredCwd}`);
    return configuredCwd;
  }

  const workspaceFolder = vscode.workspace.workspaceFolders?.[0];
  if (workspaceFolder) {
    const workspacePath = workspaceFolder.uri.fsPath;
    outputChannel.appendLine(`[INFO] Using workspace folder as working directory: ${workspacePath}`);
    return workspacePath;
  }

  outputChannel.appendLine(`[INFO] Using extension path as working directory: ${context.extensionPath}`);
  return context.extensionPath;
}

/**
 * Create the language client
 * @param {vscode.ExtensionContext} context
 * @returns {Promise<LanguageClient>}
 */
async function createClient(context) {
  outputChannel.appendLine('[INFO] Creating language client...');

  const config = readConfiguration();
  const cwd = resolveWorkingDirectory(context, config.cwd);

  // Determine server command
  let serverCommand = config.command;
  if (!serverCommand) {
    serverCommand = getServerPath(context);
    if (!serverCommand) {
      const message = 'Vizsla Language Server binary not found. Please build the server separately, install it in your PATH, or configure "vizslaLsp.server.command".';
      outputChannel.appendLine(`[ERROR] ${message}`);
      vscode.window.showErrorMessage(message);
      throw new Error(message);
    }
  } else {
    outputChannel.appendLine(`[INFO] Using custom server command: ${serverCommand}`);
  }

  const serverArgs = [...config.args, ...config.additionalArgs];
  outputChannel.appendLine(`[INFO] Server command: ${serverCommand}`);
  outputChannel.appendLine(`[INFO] Server args: ${JSON.stringify(serverArgs)}`);
  outputChannel.appendLine(`[INFO] Working directory: ${cwd}`);

  // Server options
  const serverOptions = {
    run: {
      command: serverCommand,
      args: serverArgs,
      options: {
        cwd,
        env: {
          ...process.env,
          RUST_BACKTRACE: '1',
          RUST_LOG: 'info'
        }
      }
    },
    debug: {
      command: serverCommand,
      args: serverArgs,
      options: {
        cwd,
        env: {
          ...process.env,
          RUST_BACKTRACE: 'full',
          RUST_LOG: 'debug'
        }
      }
    }
  };

  // Client options
  const clientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'verilog' },
      { scheme: 'file', language: 'systemverilog' }
    ],
    synchronize: {
      configurationSection: 'vizsla',
      fileEvents: [
        vscode.workspace.createFileSystemWatcher('**/*.v'),
        vscode.workspace.createFileSystemWatcher('**/*.vh'),
        vscode.workspace.createFileSystemWatcher('**/*.sv'),
        vscode.workspace.createFileSystemWatcher('**/*.svh'),
        vscode.workspace.createFileSystemWatcher('**/*.svi')
      ]
    },
    outputChannel,
    traceOutputChannel: outputChannel,
    revealOutputChannelOn: 4, // Never automatically reveal
    initializationOptions: {},
    // Add trace setting
    ...(config.trace !== 'off' && { trace: config.trace })
  };

  outputChannel.appendLine('[INFO] Creating LanguageClient instance...');
  const languageClient = new LanguageClient(
    'vizslaLsp',
    'Vizsla Language Server',
    serverOptions,
    clientOptions
  );

  return languageClient;
}

/**
 * Start the language client
 * @param {vscode.ExtensionContext} context
 */
async function startClient(context) {
  try {
    outputChannel.appendLine('[INFO] Starting language server...');
    client = await createClient(context);
    await client.start();
    outputChannel.appendLine('[INFO] Language server started successfully');
    vscode.window.showInformationMessage('Vizsla Language Server started');
  } catch (error) {
    outputChannel.appendLine(`[ERROR] Failed to start language server: ${error.message}`);
    outputChannel.appendLine(`[ERROR] ${error.stack}`);
    vscode.window.showErrorMessage(`Failed to start Vizsla Language Server: ${error.message}`);
  }
}

/**
 * Stop the language client
 */
async function stopClient() {
  if (!client) {
    return;
  }

  outputChannel.appendLine('[INFO] Stopping language server...');
  try {
    await client.stop();
    outputChannel.appendLine('[INFO] Language server stopped');
  } catch (error) {
    outputChannel.appendLine(`[ERROR] Error stopping language server: ${error.message}`);
  }
  client = undefined;
}

/**
 * Restart the language client
 * @param {vscode.ExtensionContext} context
 */
async function restartClient(context) {
  outputChannel.appendLine('[INFO] Restarting language server...');
  await stopClient();
  await startClient(context);
}

/**
 * Activate the extension
 * @param {vscode.ExtensionContext} context
 */
async function activate(context) {
  // Create output channel
  outputChannel = vscode.window.createOutputChannel('Vizsla Language Server');
  context.subscriptions.push(outputChannel);

  outputChannel.appendLine('[INFO] Vizsla extension activating...');
  outputChannel.appendLine(`[INFO] Extension path: ${context.extensionPath}`);
  outputChannel.appendLine(`[INFO] Platform: ${process.platform}-${process.arch}`);
  outputChannel.appendLine(`[INFO] VS Code version: ${vscode.version}`);

  // Register restart command
  const restartCommand = vscode.commands.registerCommand('vizsla.restartServer', async () => {
    outputChannel.appendLine('[INFO] Restart command triggered');
    await restartClient(context);
  });
  context.subscriptions.push(restartCommand);

  // Start the client
  await startClient(context);

  outputChannel.appendLine('[INFO] Vizsla extension activated');
}

/**
 * Deactivate the extension
 */
async function deactivate() {
  if (outputChannel) {
    outputChannel.appendLine('[INFO] Vizsla extension deactivating...');
  }
  await stopClient();
  if (outputChannel) {
    outputChannel.appendLine('[INFO] Vizsla extension deactivated');
  }
}

module.exports = {
  activate,
  deactivate
};
