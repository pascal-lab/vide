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

const vscodeCommit = "e4c7e7b1d6d060162f4aa7f8225271b67ce1df75";
const testRunnerDataDir = path.resolve(__dirname, ".vscode-test-web");

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
