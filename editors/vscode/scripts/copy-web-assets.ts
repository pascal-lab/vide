import * as fs from "node:fs";
import * as path from "node:path";

const vscodeDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(vscodeDir, "..", "..");
const sourceDir = path.join(repoRoot, "website", "playground", "public", "wasm");
const targetDir = path.join(vscodeDir, "dist", "browser", "wasm");
const requiredFiles = ["vide-lsp.js", "vide-core.js", "vide-core.wasm"];
const buildWasmCommand = "npm --prefix website --workspace playground run build:wasm";

if (!fs.existsSync(sourceDir)) {
  throw new Error(
    `Missing ${sourceDir}. Run \`${buildWasmCommand}\` first.`,
  );
}

for (const fileName of requiredFiles) {
  const sourcePath = path.join(sourceDir, fileName);
  if (!fs.existsSync(sourcePath)) {
    throw new Error(
      `Missing ${sourcePath}. Run \`${buildWasmCommand}\` first.`,
    );
  }
}

fs.mkdirSync(targetDir, { recursive: true });
for (const fileName of requiredFiles) {
  fs.copyFileSync(path.join(sourceDir, fileName), path.join(targetDir, fileName));
}
