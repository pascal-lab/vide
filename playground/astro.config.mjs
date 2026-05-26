import { defineConfig } from "astro/config";

function normalizeBase(value) {
  if (!value || value === "/") {
    return "/";
  }
  return `/${value.replace(/^\/+|\/+$/g, "")}/`;
}

export default defineConfig({
  site: process.env.ASTRO_SITE || undefined,
  base: normalizeBase(process.env.ASTRO_BASE),
  vite: {
    worker: {
      format: "es",
    },
  },
});
