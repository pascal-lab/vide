import { defineConfig } from "vite";
import { copyFileSync, mkdirSync, readdirSync } from "node:fs";
import { resolve } from "node:path";

const monacoNlsLocales = new Set(["cs", "de", "es", "fr", "it", "ja", "ko", "pl", "pt-br", "ru", "tr", "zh-cn", "zh-tw"]);

export default defineConfig({
  plugins: [
    {
      name: "copy-monaco-nls",
      closeBundle() {
        const monacoRoot = resolve("node_modules", "monaco-editor", "esm");
        const outRoot = resolve("dist", "embed");
        mkdirSync(outRoot, { recursive: true });
        for (const fileName of readdirSync(monacoRoot)) {
          const match = /^nls\.messages\.(.+)\.js$/.exec(fileName);
          if (match && monacoNlsLocales.has(match[1])) {
            copyFileSync(resolve(monacoRoot, fileName), resolve(outRoot, fileName));
          }
        }
      },
    },
  ],
  build: {
    target: "es2022",
    outDir: "dist/embed",
    emptyOutDir: true,
    lib: {
      entry: "src/components/vide-lab.ts",
      name: "VideLab",
      formats: ["es"],
      fileName: (format) => `vide-lab.${format}.js`,
    },
  },
  worker: {
    format: "es",
  },
});
