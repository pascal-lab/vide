const vscode = require('vscode');
const path = require('path');
const fs = require('fs');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

const RUN_QIHE_ANALYSIS_COMMAND = 'vizsla.runQiheAnalysis';
const RUN_QIHE_ANALYSIS_REQUEST = 'vizsla.server.runQiheAnalysis';
const QIHE_STATUS_NOTIFICATION = 'vizsla/qiheStatus';
const QIHE_ANALYSIS_ICON = '$(beaker)';

/** @type {LanguageClient | undefined} */
let client;

/** @type {vscode.OutputChannel} */
let outputChannel;

/** @type {vscode.StatusBarItem | undefined} */
let qiheStatusBarItem;

const activeQiheTokens = new Set();
let qiheStatusHideTimer;
const qiheProgressNotifications = new Map();

function clearQiheStatusHideTimer() {
  if (qiheStatusHideTimer) {
    clearTimeout(qiheStatusHideTimer);
    qiheStatusHideTimer = undefined;
  }
}

function createQiheStatusBarItem() {
  const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  item.name = 'Vizsla Qihe';
  item.command = RUN_QIHE_ANALYSIS_COMMAND;
  item.hide();
  return item;
}

function showRunningQiheStatus(message) {
  if (!qiheStatusBarItem) {
    return;
  }

  clearQiheStatusHideTimer();
  qiheStatusBarItem.text = `${QIHE_ANALYSIS_ICON} Qihe`;
  qiheStatusBarItem.tooltip = message ? `Qihe running: ${message}` : 'Qihe analysis is running';
  qiheStatusBarItem.show();
}

function showCompletedQiheStatus(message) {
  if (!qiheStatusBarItem) {
    return;
  }

  clearQiheStatusHideTimer();
  qiheStatusBarItem.text = `${QIHE_ANALYSIS_ICON} Qihe`;
  qiheStatusBarItem.tooltip = message || 'Qihe analysis finished';
  qiheStatusBarItem.show();
  qiheStatusHideTimer = setTimeout(() => {
    qiheStatusBarItem?.hide();
    qiheStatusHideTimer = undefined;
  }, 4000);
}

function showFailedQiheStatus(message) {
  if (!qiheStatusBarItem) {
    return;
  }

  clearQiheStatusHideTimer();
  qiheStatusBarItem.text = `${QIHE_ANALYSIS_ICON} Qihe`;
  qiheStatusBarItem.tooltip = message || 'Qihe analysis failed';
  qiheStatusBarItem.show();
  qiheStatusHideTimer = setTimeout(() => {
    qiheStatusBarItem?.hide();
    qiheStatusHideTimer = undefined;
  }, 6000);
}

function hideQiheStatus() {
  clearQiheStatusHideTimer();
  qiheStatusBarItem?.hide();
}

function startQiheNotification(token, message) {
  if (qiheProgressNotifications.has(token)) {
    return;
  }

  let resolveProgress;
  const progressPromise = new Promise((resolve) => {
    resolveProgress = resolve;
  });

  qiheProgressNotifications.set(token, {
    resolve: resolveProgress,
  });

  vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'Running Qihe analysis',
    },
    async (progress) => {
      if (message) {
        progress.report({ message });
      }
      await progressPromise;
    }
  );
}

function finishQiheNotification(token) {
  const entry = qiheProgressNotifications.get(token);
  if (!entry) {
    return;
  }

  qiheProgressNotifications.delete(token);
  entry.resolve();
}

function handleQiheStatus(params) {
  const token = typeof params?.token === 'string' ? params.token : undefined;
  if (!token) {
    return;
  }

  switch (params?.state) {
    case 'begin':
      activeQiheTokens.add(token);
      showRunningQiheStatus(params.message);
      startQiheNotification(token, params.message);
      break;
    case 'end':
      activeQiheTokens.delete(token);
      finishQiheNotification(token);
      if (activeQiheTokens.size === 0) {
        showCompletedQiheStatus(params.message);
      }
      break;
    case 'failed':
      activeQiheTokens.delete(token);
      finishQiheNotification(token);
      if (activeQiheTokens.size === 0) {
        showFailedQiheStatus(params.message);
      }
      break;
    default:
      break;
  }
}

function registerQiheNotifications(languageClient) {
  languageClient.onNotification(QIHE_STATUS_NOTIFICATION, handleQiheStatus);
}

async function runQiheAnalysis() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage('Open a Verilog or SystemVerilog file first.');
    return;
  }

  const languageId = editor.document.languageId;
  if (languageId !== 'verilog' && languageId !== 'systemverilog') {
    vscode.window.showWarningMessage('Qihe analysis is only available for Verilog files.');
    return;
  }

  if (!client) {
    vscode.window.showErrorMessage('Vizsla language server is not running.');
    return;
  }

  const qiheConfig = readQiheConfiguration();
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(editor.document.uri);
  const payload = {
    uri: editor.document.uri.toString(),
    cwd: workspaceFolder?.uri.fsPath,
  };

  outputChannel.appendLine(
    `[INFO] Running Qihe analysis: ${JSON.stringify(payload)} with config ${JSON.stringify(qiheConfig)}`
  );

  try {
    await client.sendRequest('workspace/executeCommand', {
      command: RUN_QIHE_ANALYSIS_REQUEST,
      arguments: [payload],
    });
  } catch (error) {
    outputChannel.appendLine(`[ERROR] Qihe analysis request failed: ${error.message}`);
    vscode.window.showErrorMessage(`Failed to run Qihe analysis: ${error.message}`);
  }
}

function registerQiheCommand(context) {
  const command = vscode.commands.registerCommand(RUN_QIHE_ANALYSIS_COMMAND, runQiheAnalysis);
  context.subscriptions.push(command);
}

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
  const config = vscode.workspace.getConfiguration('vizsla');
  const command = config.get('server.command');
  const args = config.get('server.args') ?? [];
  const additionalArgs = config.get('server.additionalArgs') ?? [];
  const cwd = config.get('server.cwd');
  const trace = config.get('trace.server') ?? 'off';

  if (!Array.isArray(args) || !Array.isArray(additionalArgs)) {
    vscode.window.showErrorMessage('vizsla server arguments settings must be arrays of strings.');
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

function readQiheConfiguration() {
  const config = vscode.workspace.getConfiguration('vizsla');
  const command = config.get('qihe.command') ?? 'qihe';
  const compileArgs = config.get('qihe.compileArgs') ?? [];
  const runArgs = config.get('qihe.runArgs') ?? ['-g', 'std'];

  return {
    command: typeof command === 'string' && command.length > 0 ? command : 'qihe',
    compileArgs,
    runArgs,
  };
}

function readInitializationOptions() {
  return {
    qihe: readQiheConfiguration(),
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
      const message = 'Vizsla Language Server binary not found. Please build the server separately, install it in your PATH, or configure "vizsla.server.command".';
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
    initializationOptions: readInitializationOptions(),
    // Add trace setting
    ...(config.trace !== 'off' && { trace: config.trace })
  };

  outputChannel.appendLine('[INFO] Creating LanguageClient instance...');
  const languageClient = new LanguageClient(
    'vizsla',
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
  activeQiheTokens.clear();
  for (const token of qiheProgressNotifications.keys()) {
    finishQiheNotification(token);
  }
  hideQiheStatus();
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
  qiheStatusBarItem = createQiheStatusBarItem();
  context.subscriptions.push(qiheStatusBarItem);

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
  registerQiheCommand(context);

  // Start the client
  await startClient(context);
  if (client) {
    registerQiheNotifications(client);
  }

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
  hideQiheStatus();
}

module.exports = {
  activate,
  deactivate
};
