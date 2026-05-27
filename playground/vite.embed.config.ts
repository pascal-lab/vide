import { defineConfig } from "vite";

export default defineConfig({
  build: {
    target: "es2022",
    outDir: "dist/embed",
    emptyOutDir: true,
    lib: {
      entry: {
        "vide-lab": "src/components/vide-lab.ts",
        "locale-zh-hans": "src/locale/zh-hans.ts",
      },
      name: "VideLab",
      formats: ["es"],
      fileName: (format, entryName) => `${entryName}.${format}.js`,
      cssFileName: "vide-playground",
    },
  },
  worker: {
    format: "es",
  },
});
