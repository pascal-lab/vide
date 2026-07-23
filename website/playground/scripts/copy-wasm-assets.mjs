import { cpSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { playgroundRoot, requireFiles, WASM_FILES, workspaceRoot } from "./script-utils.mjs";

const wasmSource = resolve(workspaceRoot, "crates", "vide-lsp-wasm", "dist");
const wasmTarget = resolve(playgroundRoot, "public", "wasm");

requireFiles(wasmSource, WASM_FILES, "Run the vide-lsp-wasm build first.");

rmSync(wasmTarget, { recursive: true, force: true });
mkdirSync(wasmTarget, { recursive: true });
cpSync(wasmSource, wasmTarget, { recursive: true });
console.log(`Copied Vide WASM playground assets to ${wasmTarget}`);
