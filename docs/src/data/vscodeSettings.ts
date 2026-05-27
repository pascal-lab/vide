import { readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import process from "node:process";

export type VscodeSettingsLocale = "en" | "zh";

type VscodeSettingDefinition = {
  default?: unknown;
  description?: string;
  markdownDescription?: string;
  enum?: string[];
  enumDescriptions?: string[];
};

type VscodePackage = {
  contributes: {
    configuration: {
      properties: Record<string, VscodeSettingDefinition>;
    };
  };
};

export type VscodeSettingEnumValue = {
  value: string;
  description?: string;
};

export type VscodeSettingRow = {
  name: string;
  defaultValue: string;
  description: string;
  enumValues?: VscodeSettingEnumValue[];
};

const docsRoot = basename(process.cwd()) === "docs" ? process.cwd() : resolve(process.cwd(), "docs");
const repoRoot = resolve(docsRoot, "..");
const extensionRoot = resolve(repoRoot, "editors/vscode");

const vscodePackage = readJson<VscodePackage>(resolve(extensionRoot, "package.json"));
const localeMessages = {
  en: readJson<Record<string, string>>(resolve(extensionRoot, "package.nls.json")),
  zh: readJson<Record<string, string>>(resolve(extensionRoot, "package.nls.zh-cn.json")),
} satisfies Record<VscodeSettingsLocale, Record<string, string>>;

export function getVscodeSettingRows(
  names: string[],
  locale: VscodeSettingsLocale,
): VscodeSettingRow[] {
  const properties = vscodePackage.contributes.configuration.properties;
  return names.map((name) => {
    const definition = properties[name];
    if (!definition) {
      throw new Error(`Unknown VS Code setting: ${name}`);
    }

    return {
      name,
      defaultValue: formatDefaultValue(definition.default),
      description: localize(
        definition.markdownDescription ?? definition.description ?? "",
        locale,
      ),
      enumValues: definition.enum?.map((value, index) => {
        const description = definition.enumDescriptions?.[index];
        return {
          value,
          description: description ? localize(description, locale) : undefined,
        };
      }),
    };
  });
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, "utf8")) as T;
}

function localize(value: string, locale: VscodeSettingsLocale): string {
  const key = /^%(.+)%$/.exec(value)?.[1];
  if (!key) {
    return value;
  }

  return localeMessages[locale][key] ?? localeMessages.en[key] ?? value;
}

function formatDefaultValue(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(formatDefaultValue).join(", ")}]`;
  }

  if (typeof value === "string") {
    return JSON.stringify(value);
  }

  return String(value);
}
