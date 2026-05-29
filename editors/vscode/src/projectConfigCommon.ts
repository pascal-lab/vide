import {
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
} from "./generated/projectConfigSchema";

export {
  PROJECT_CONFIG_SCHEMA_PATH,
  PROJECT_CONFIG_SCHEMA_URL,
  PROJECT_CONFIG_SCHEMA_VERSION,
} from "./generated/projectConfigSchema";

export const PROJECT_CONFIG_FILE_NAME = "vide.toml";
export const PROJECT_CONFIG_FILE_NAMES = [PROJECT_CONFIG_FILE_NAME] as const;
export const PROJECT_SOURCE_FILE_EXTENSIONS = [
  ".v",
  ".sv",
  ".vh",
  ".svh",
  ".svi",
] as const;
export const PROJECT_SOURCE_FILE_GLOB = "**/*.{v,sv,vh,svh,svi}";

export const DEFAULT_PROJECT_CONFIG_TEXT = `#:schema ${PROJECT_CONFIG_SCHEMA_URL}
sources = []

# include_dirs = ["include"]
# defines = ["SYNTHESIS"]
# top_modules = ["top"]
# libraries = ["../common_cells"]
# exclude = ["build/**"]
`;

export function isProjectConfigFileName(fileName: string): boolean {
  return PROJECT_CONFIG_FILE_NAMES.includes(
    fileName as (typeof PROJECT_CONFIG_FILE_NAMES)[number],
  );
}

export function isProjectSourceFileName(fileName: string): boolean {
  return PROJECT_SOURCE_FILE_EXTENSIONS.includes(
    lowerCaseExtension(fileName) as (typeof PROJECT_SOURCE_FILE_EXTENSIONS)[number],
  );
}

function lowerCaseExtension(fileName: string): string {
  const normalized = fileName.replace(/\\/g, "/");
  const lastSlash = normalized.lastIndexOf("/");
  const baseName = lastSlash >= 0 ? normalized.slice(lastSlash + 1) : normalized;
  const extensionIndex = baseName.lastIndexOf(".");
  return extensionIndex >= 0 ? baseName.slice(extensionIndex).toLowerCase() : "";
}
