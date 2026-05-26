import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { repoRoot, workspaceRoot } from "./script-utils.mjs";

const source = resolve(repoRoot, "public", "wasm");
const target = resolve(workspaceRoot, "docs", "public", "vide-lab", "wasm");
const requiredFiles = ["vide-lsp.js", "vide-core.js", "vide-core.wasm"];

for (const file of requiredFiles) {
  const path = resolve(source, file);
  if (!existsSync(path)) {
    throw new Error(`Missing ${path}. Run npm run build:wasm in the playground package first.`);
  }
}

mkdirSync(resolve(target, ".."), { recursive: true });
rmSync(target, { recursive: true, force: true });
cpSync(source, target, { recursive: true, force: true });
console.log(`Copied Vide WASM docs assets to ${target}`);
