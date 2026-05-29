import "mocha/mocha";

declare const mocha: {
  setup(options: Record<string, unknown>): void;
  run(callback: (failures: number) => void): void;
};

mocha.setup({
  ui: "tdd",
  reporter: undefined,
  timeout: 30_000,
});

// Keep this after mocha.setup; ESM imports would be hoisted too early.
declare const require: (id: string) => unknown;
require("./activation.test");

export function run(): Promise<void> {
  return new Promise((resolve, reject) => {
    mocha.run((failures) => {
      if (failures > 0) {
        reject(new Error(`${failures} test(s) failed.`));
      } else {
        resolve();
      }
    });
  });
}
