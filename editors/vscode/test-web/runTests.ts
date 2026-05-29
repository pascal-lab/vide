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

async function main(): Promise<void> {
  await runTests({
    browserType: "chromium",
    extensionDevelopmentPath,
    extensionTestsPath,
    headless: true,
    quality: "stable",
  });
}

void main();
