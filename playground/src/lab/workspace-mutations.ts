import type { VideScenario } from "../types";
import { fileName, languageIdForPath, normalizeWorkspacePath } from "./workspace";

export interface WorkspaceMutation {
  scenario: VideScenario;
  activePath: string;
}

export function cloneScenario(scenario: VideScenario): VideScenario {
  return {
    ...scenario,
    files: scenario.files.map((file) => ({ ...file })),
  };
}

export function createFileScenario(
  activeScenario: VideScenario,
  currentFiles: VideScenario["files"],
  path: string,
): WorkspaceMutation {
  return {
    scenario: {
      ...activeScenario,
      files: [
        ...currentFiles,
        {
          path,
          source: defaultSourceForPath(path),
        },
      ],
      entryFile: path,
    },
    activePath: path,
  };
}

export function renameFileScenario(
  activeScenario: VideScenario,
  currentFiles: VideScenario["files"],
  fromPath: string,
  nextPath: string,
): WorkspaceMutation {
  const files = currentFiles.map((file) =>
    file.path === fromPath
      ? {
          ...file,
          path: nextPath,
          languageId: file.languageId && languageIdForPath(nextPath) === "plaintext" ? file.languageId : undefined,
        }
      : file,
  );
  const entry = normalizeWorkspacePath(activeScenario.entryFile) === fromPath ? nextPath : activeScenario.entryFile;
  return { scenario: { ...activeScenario, files, entryFile: entry }, activePath: nextPath };
}

export function deleteFileScenario(
  activeScenario: VideScenario,
  currentFiles: VideScenario["files"],
  path: string,
): WorkspaceMutation | { error: string } {
  if (currentFiles.length <= 1) {
    return { error: "The workspace must keep at least one file." };
  }
  const deletedIndex = currentFiles.findIndex((file) => file.path === path);
  if (deletedIndex < 0) {
    return { error: "The file is no longer in the workspace." };
  }

  const files = currentFiles.filter((file) => file.path !== path);
  const fallback = files[Math.min(deletedIndex, files.length - 1)] ?? files[0];
  const entry = normalizeWorkspacePath(activeScenario.entryFile) === path ? fallback.path : activeScenario.entryFile;
  return { scenario: { ...activeScenario, files, entryFile: entry }, activePath: fallback.path };
}

export function defaultNewFilePath(hasWorkspacePath: (path: string) => boolean): string {
  for (let index = 1; index < 1000; index += 1) {
    const suffix = index === 1 ? "" : `_${index}`;
    const candidate = `rtl/new_module${suffix}.sv`;
    if (!hasWorkspacePath(candidate)) {
      return candidate;
    }
  }

  return "new_file.sv";
}

function defaultSourceForPath(path: string): string {
  const languageId = languageIdForPath(path);
  if (languageId !== "verilog" && languageId !== "systemverilog") {
    return "";
  }

  return `module ${moduleNameForPath(path)};
endmodule
`;
}

function moduleNameForPath(path: string): string {
  const withoutExtension = fileName(path).replace(/\.[^.]+$/, "");
  const normalized = withoutExtension.replace(/\W+/g, "_").replace(/^_+|_+$/g, "");
  if (!normalized) {
    return "new_module";
  }
  return /^\d/.test(normalized) ? `m_${normalized}` : normalized;
}
