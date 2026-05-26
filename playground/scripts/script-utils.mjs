import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const workspaceRoot = resolve(repoRoot, "..");

export function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    stdio: "inherit",
    shell: false,
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}`);
  }
}

export function output(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    shell: false,
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const stderr = result.stderr?.trim();
    throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}${stderr ? `: ${stderr}` : ""}`);
  }
  return result.stdout.trim();
}

export function tryRun(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env: process.env,
    stdio: "ignore",
    shell: false,
    ...options,
  });
  return !result.error && result.status === 0;
}

export function git(args, options = {}) {
  run("git", args, options);
}

export function gitOutput(args, options = {}) {
  return output("git", args, options);
}

export function commitPresent(repositoryPath, sha) {
  return !!sha && tryRun("git", ["-C", repositoryPath, "rev-parse", "--verify", `${sha}^{commit}`]);
}

export function findFirstFile(root, extension) {
  if (!existsSync(root)) {
    return undefined;
  }

  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = resolve(root, entry.name);
    if (entry.isDirectory()) {
      const nested = findFirstFile(path, extension);
      if (nested) {
        return nested;
      }
    } else if (entry.isFile() && entry.name.endsWith(extension)) {
      return path;
    }
  }

  return undefined;
}

export function directoryExists(path) {
  return existsSync(path) && statSync(path).isDirectory();
}
