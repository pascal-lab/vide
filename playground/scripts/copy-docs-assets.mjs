import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { repoRoot, workspaceRoot } from "./script-utils.mjs";

const docsAssetRoot = resolve(workspaceRoot, "docs", "public", "vide-lab");
const embedSource = resolve(repoRoot, "dist", "embed");
const wasmSource = resolve(repoRoot, "public", "wasm");
const wasmTarget = resolve(docsAssetRoot, "wasm");

const requiredEmbedFiles = ["vide-lab.es.js", "locale-zh-hans.es.js", "vide-playground.css"];
const requiredWasmFiles = ["vide-lsp.js", "vide-core.js", "vide-core.wasm"];

function requireFiles(root, files, hint) {
  for (const file of files) {
    const path = resolve(root, file);
    if (!existsSync(path)) {
      throw new Error(`Missing ${path}. ${hint}`);
    }
  }
}

requireFiles(embedSource, requiredEmbedFiles, "Run npm run build:embed in the playground package first.");

rmSync(docsAssetRoot, { recursive: true, force: true });
mkdirSync(docsAssetRoot, { recursive: true });
cpSync(embedSource, docsAssetRoot, { recursive: true });
console.log(`Copied Vide Lab embed assets to ${docsAssetRoot}`);

requireFiles(wasmSource, requiredWasmFiles, "Run npm run build:wasm in the playground package first.");
rmSync(wasmTarget, { recursive: true, force: true });
cpSync(wasmSource, wasmTarget, { recursive: true });
console.log(`Copied Vide WASM docs assets to ${wasmTarget}`);
