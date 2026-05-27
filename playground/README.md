# Vide Playground

Browser playground for Vide with an Astro page and an embeddable docs widget.

## Local Build

`build:wasm` expects an activated Emscripten SDK environment. CI uses
Emscripten SDK 5.0.2; local builds should use the same version unless you are
intentionally testing a toolchain update.

```sh
npm install
npm run build:wasm
npm run dev
```

Open `http://127.0.0.1:5177/`.

The docs-style embedded widget example is available at `http://127.0.0.1:5177/embed-example/`.
