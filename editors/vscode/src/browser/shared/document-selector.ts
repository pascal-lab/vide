import {
  FUSE_SOC_CORE_FILE_GLOB,
  PROJECT_CONFIG_FILE_GLOB,
  PROJECT_CONFIG_FILE_NAME,
} from "../../projectConfigCommon";

export function videDocumentSelector(rootUri: string): Array<{
  scheme: "file";
  language?: "verilog" | "systemverilog";
  pattern?: string;
}> {
  const rootPath = new URL(rootUri).pathname.replace(/\/+$/, "");
  const pattern = rootPath ? `${decodeURIComponent(rootPath)}/**` : undefined;
  const manifestPattern = rootPath
    ? `${decodeURIComponent(rootPath)}/**/${PROJECT_CONFIG_FILE_NAME}`
    : PROJECT_CONFIG_FILE_GLOB;
  const corePattern = rootPath
    ? `${decodeURIComponent(rootPath)}/**/*.core`
    : FUSE_SOC_CORE_FILE_GLOB;

  return [
    { scheme: "file", language: "systemverilog", pattern },
    { scheme: "file", language: "verilog", pattern },
    { scheme: "file", pattern: manifestPattern },
    { scheme: "file", pattern: corePattern },
  ];
}
