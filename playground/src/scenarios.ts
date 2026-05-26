import type { VizslaScenario } from "./types";

type ScenarioFileDefinition = Omit<VizslaScenario["files"][number], "source">;

interface ScenarioDefinition {
  id: string;
  order: number;
  label: string;
  entryFile: string;
  description: string;
  files: ScenarioFileDefinition[];
}

const scenarioDefinitions = import.meta.glob("./scenarios/*/scenario.json", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;

const scenarioSources = import.meta.glob("./scenarios/**/*", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;

function normalizeScenarioPath(path: string): string {
  return path.replace(/\\/g, "/").replace(/^\/+/, "");
}

function scenarioDirectory(metadataPath: string): string {
  const match = /^\.\/scenarios\/([^/]+)\/scenario\.json$/.exec(metadataPath);
  if (match === null) {
    throw new Error(`Unexpected scenario metadata path: ${metadataPath}`);
  }

  return match[1];
}

function scenarioDefinition(metadataPath: string, rawDefinition: string): ScenarioDefinition {
  const directory = scenarioDirectory(metadataPath);
  const definition = JSON.parse(rawDefinition) as ScenarioDefinition;
  if (definition.id !== directory) {
    throw new Error(`Scenario metadata id mismatch: expected ${directory}, found ${definition.id}`);
  }

  return definition;
}

function scenarioSource(id: string, path: string): string {
  const key = `./scenarios/${id}/${normalizeScenarioPath(path)}`;
  const source = scenarioSources[key];
  if (source === undefined) {
    throw new Error(`Missing scenario source: ${key}`);
  }

  return source;
}

function loadScenario(definition: ScenarioDefinition): VizslaScenario {
  return {
    id: definition.id,
    label: definition.label,
    entryFile: normalizeScenarioPath(definition.entryFile),
    description: definition.description,
    files: definition.files.map((file) => ({
      ...file,
      path: normalizeScenarioPath(file.path),
      source: scenarioSource(definition.id, file.path),
    })),
  };
}

export const SCENARIOS: VizslaScenario[] = Object.entries(scenarioDefinitions)
  .map(([metadataPath, rawDefinition]) => scenarioDefinition(metadataPath, rawDefinition))
  .sort((left, right) => left.order - right.order || left.label.localeCompare(right.label))
  .map(loadScenario);

export function getScenario(id: string | null | undefined): VizslaScenario {
  return SCENARIOS.find((scenario) => scenario.id === id) ?? SCENARIOS[0];
}
