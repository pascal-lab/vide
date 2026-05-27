import createVideModule from "./vide-core.js";

export async function createVideLspEngine(options = {}) {
  const assetSearch = new URL(import.meta.url).search;
  const module = await createVideModule({
    locateFile(path) {
      const url = new URL(path.endsWith(".wasm") ? "vide-core.wasm" : path, import.meta.url);
      url.search = assetSearch;
      return url.href;
    },
  });
  writeWorkspace(module, options.rootUri ?? "file:///workspace", options.workspaceFiles ?? []);

  return {
    send(message) {
      return drainMessages(module, callJson(module, "vide_lsp_message", JSON.stringify(message)));
    },
    poll() {
      return drainMessages(module, []);
    },
    writeFile(path, text) {
      writeWorkspace(module, options.rootUri ?? "file:///workspace", [{ path, text }]);
    },
    reset() {
      module._vide_lsp_reset();
    },
  };
}

function drainMessages(module, initialMessages) {
  const messages = Array.isArray(initialMessages) ? [...initialMessages] : [];
  for (let index = 0; index < 16; index += 1) {
    const polled = callJson(module, "vide_lsp_poll", "null");
    if (!Array.isArray(polled) || polled.length === 0) {
      break;
    }
    messages.push(...polled);
  }
  return messages;
}

function writeWorkspace(module, rootUri, files) {
  if (!Array.isArray(files)) {
    throw new Error("workspaceFiles must be an array.");
  }
  const rootPath = pathFromFileUri(rootUri);
  for (const file of files) {
    if (!file || typeof file.path !== "string" || typeof file.text !== "string") {
      throw new Error("workspaceFiles entries must contain path and text.");
    }
    const normalized = file.path.replace(/\\/g, "/").replace(/^\/+/, "");
    if (normalized.includes("..")) {
      throw new Error(`Refusing workspace path outside /workspace: ${file.path}`);
    }
    callVoid(module, "vide_lsp_write_file", `${rootPath}/${normalized}`, file.text);
  }
}

function pathFromFileUri(uri) {
  const url = new URL(uri);
  if (url.protocol !== "file:") {
    throw new Error(`Unsupported workspace root URI: ${uri}`);
  }
  const path = decodeURIComponent(url.pathname).replace(/\/+$/, "");
  return path || "/workspace";
}

function callJson(module, name, payload) {
  const byteLength = module.lengthBytesUTF8(payload);
  const ptr = module._malloc(byteLength + 1);

  try {
    module.stringToUTF8(payload, ptr, byteLength + 1);
    const fn = module[`_${name}`];
    if (typeof fn !== "function") {
      throw new Error(`Vide WASM export not found: ${name}`);
    }
    const resultPtr = fn(ptr, byteLength);
    try {
      const value = JSON.parse(module.UTF8ToString(resultPtr));
      if (value && typeof value.error === "string") {
        throw new Error(value.error);
      }
      return value;
    } finally {
      module._vide_free_string(resultPtr);
    }
  } finally {
    module._free(ptr);
  }
}

function callVoid(module, name, path, text) {
  const pathLength = module.lengthBytesUTF8(path);
  const textLength = module.lengthBytesUTF8(text);
  const pathPtr = module._malloc(pathLength + 1);
  const textPtr = module._malloc(textLength + 1);

  try {
    module.stringToUTF8(path, pathPtr, pathLength + 1);
    module.stringToUTF8(text, textPtr, textLength + 1);
    const fn = module[`_${name}`];
    if (typeof fn !== "function") {
      throw new Error(`Vide WASM export not found: ${name}`);
    }
    const resultPtr = fn(pathPtr, pathLength, textPtr, textLength);
    try {
      const value = JSON.parse(module.UTF8ToString(resultPtr));
      if (value && typeof value.error === "string") {
        throw new Error(value.error);
      }
    } finally {
      module._vide_free_string(resultPtr);
    }
  } finally {
    module._free(pathPtr);
    module._free(textPtr);
  }
}
