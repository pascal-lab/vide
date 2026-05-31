import { readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import process from "node:process";

import { USER_CONFIG_SETTINGS } from "../../../editors/vscode/src/generated/configuration";

export type UserConfigSettingsLocale = "en" | "zh";

type VscodeSettingDefinition = {
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

export type UserConfigSettingEnumValue = {
  value: string;
  description?: string;
};

export type UserConfigSettingRow = {
  name: string;
  vscodeName: string;
  defaultValue: string;
  description: string;
  enumValues?: UserConfigSettingEnumValue[];
};

const docsRoot = basename(process.cwd()) === "docs" ? process.cwd() : resolve(process.cwd(), "docs");
const repoRoot = resolve(docsRoot, "..");
const extensionRoot = resolve(repoRoot, "editors/vscode");

const vscodePackage = readJson<VscodePackage>(resolve(extensionRoot, "package.json"));
const localeMessages = {
  en: readJson<Record<string, string>>(resolve(extensionRoot, "package.nls.json")),
  zh: readJson<Record<string, string>>(resolve(extensionRoot, "package.nls.zh-cn.json")),
} satisfies Record<UserConfigSettingsLocale, Record<string, string>>;

const settingsByPath = new Map(
  USER_CONFIG_SETTINGS.map((setting) => [setting.path.join("."), setting]),
);

export function getUserConfigSettingRows(
  names: string[],
  locale: UserConfigSettingsLocale,
): UserConfigSettingRow[] {
  const properties = vscodePackage.contributes.configuration.properties;
  return names.map((name) => {
    const setting = settingsByPath.get(name);
    if (!setting) {
      throw new Error(`Unknown user config setting: ${name}`);
    }

    const definition = properties[setting.vscodeKey];

    return {
      name,
      vscodeName: setting.vscodeKey,
      defaultValue: formatDefaultValue(setting.defaultValue),
      description: localizeKey(setting.markdownDescriptionKey ?? setting.descriptionKey, locale),
      enumValues: definition?.enum?.map((value, index) => {
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

function localizeKey(key: string, locale: UserConfigSettingsLocale): string {
  return localeMessages[locale][key] ?? localeMessages.en[key] ?? key;
}

function localize(value: string, locale: UserConfigSettingsLocale): string {
  const key = /^%(.+)%$/.exec(value)?.[1];
  if (!key) {
    return value;
  }

  return localizeKey(key, locale);
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
