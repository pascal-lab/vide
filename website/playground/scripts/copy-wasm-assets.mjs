import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { playgroundRoot, workspaceRoot } from "./script-utils.mjs";

const wasmSource = resolve(workspaceRoot, "crates", "vide-lsp-wasm", "dist");
const wasmTarget = resolve(playgroundRoot, "public", "wasm");
const requiredWasmFiles = ["vide-lsp.js", "vide-core.js", "vide-core.wasm"];

for (const file of requiredWasmFiles) {
  const path = resolve(wasmSource, file);
  if (!existsSync(path)) {
    throw new Error(`Missing ${path}. Run the vide-lsp-wasm build first.`);
  }
}

rmSync(wasmTarget, { recursive: true, force: true });
mkdirSync(wasmTarget, { recursive: true });
cpSync(wasmSource, wasmTarget, { recursive: true });
console.log(`Copied Vide WASM playground assets to ${wasmTarget}`);
