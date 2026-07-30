import * as fs from "node:fs";
import * as path from "node:path";

import { runTests } from "@vscode/test-web";

const extensionDevelopmentPath = path.resolve(__dirname, "..");
const extensionTestsPath = path.join(
  extensionDevelopmentPath,
  "dist",
  "test-web",
  "suite",
  "index.js",
);

const vscodeCommit = readVscodeCommit();
const testRunnerDataDir = path.resolve(__dirname, ".vscode-test-web");

function readVscodeCommit(): string {
  const packageJson = JSON.parse(
    fs.readFileSync(path.resolve(__dirname, "package.json"), "utf8"),
  ) as { config?: { vscodeWebCommit?: unknown } };
  const commit = packageJson.config?.vscodeWebCommit;
  if (typeof commit !== "string" || !/^[0-9a-f]{40}$/.test(commit)) {
    throw new Error(
      "test-web/package.json config.vscodeWebCommit must be a 40-character lowercase Git commit",
    );
  }
  return commit;
}

async function main(): Promise<void> {
  await runTests({
    browserType: "chromium",
    commit: vscodeCommit,
    extensionDevelopmentPath,
    extensionTestsPath,
    headless: true,
    quality: "stable",
    testRunnerDataDir,
  });
}

void main();
