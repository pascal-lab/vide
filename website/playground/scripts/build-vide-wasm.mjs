import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { delimiter, dirname, resolve } from "node:path";
import { repoRoot, run, workspaceRoot } from "./script-utils.mjs";

if (process.argv.slice(2).some((arg) => arg !== "--")) {
  throw new Error(`build:wasm does not accept arguments.`);
}

const emscriptenRoot = findEmscriptenRoot();
const emsdkRoot = process.env.EMSDK ? resolve(process.env.EMSDK) : resolve(emscriptenRoot, "..", "..");
const buildEnv = {
  ...process.env,
  EMSDK: process.env.EMSDK ?? emsdkRoot,
  PATH: [emscriptenRoot, emsdkRoot, process.env.PATH].filter(Boolean).join(delimiter),
};
const emConfig = resolve(emsdkRoot, ".emscripten");
if (!buildEnv.EM_CONFIG && existsSync(emConfig)) {
  buildEnv.EM_CONFIG = emConfig;
}

run("rustup", ["target", "add", "wasm32-unknown-emscripten"], { env: buildEnv });
run("ninja", ["--version"], { env: buildEnv });

const linkArgs = [
  "-C", "link-arg=-sENVIRONMENT=web,worker",
  "-C", "link-arg=-sMODULARIZE=1",
  "-C", "link-arg=-sEXPORT_ES6=1",
  "-C", "link-arg=-sEXPORT_NAME=createVideModule",
  "-C", "link-arg=-sEXPORTED_RUNTIME_METHODS=['UTF8ToString','stringToUTF8','lengthBytesUTF8']",
  "-C", "link-arg=-sEXPORTED_FUNCTIONS=['_malloc','_free','_vide_lsp_message','_vide_lsp_poll','_vide_lsp_write_file','_vide_lsp_reset','_vide_free_string']",
];

Object.assign(buildEnv, {
  EMSCRIPTEN_CMAKE_TOOLCHAIN_FILE: resolve(emscriptenRoot, "cmake", "Modules", "Platform", "Emscripten.cmake"),
  CMAKE_GENERATOR_wasm32_unknown_emscripten: "Ninja",
  EMCMAKE_wasm32_unknown_emscripten: emscriptenTool("emcmake"),
  EMMAKE_wasm32_unknown_emscripten: emscriptenTool("emmake"),
  CC_wasm32_unknown_emscripten: emscriptenTool("emcc"),
  CXX_wasm32_unknown_emscripten: emscriptenTool("em++"),
  AR_wasm32_unknown_emscripten: emscriptenTool("emar"),
  CARGO_TARGET_WASM32_UNKNOWN_EMSCRIPTEN_LINKER: emscriptenTool("emcc"),
  RUSTFLAGS: linkArgs.join(" "),
});

const crateManifest = resolve(workspaceRoot, "crates", "vide-lsp-wasm", "Cargo.toml");
run("cargo", ["build", "--manifest-path", crateManifest, "--target", "wasm32-unknown-emscripten", "--release"], {
  env: buildEnv,
});

const targetRoot = resolve(workspaceRoot, "target", "wasm32-unknown-emscripten", "release");
const coreJs = resolve(targetRoot, "vide-lsp-wasm.js");
const coreWasm = resolve(targetRoot, "vide_lsp_wasm.wasm");
assertFile(coreJs, "Emscripten JavaScript output");
assertFile(coreWasm, "Emscripten WASM output");

const outWasmRoot = resolve(repoRoot, "public", "wasm");
mkdirSync(outWasmRoot, { recursive: true });
copyFileSync(coreJs, resolve(outWasmRoot, "vide-core.js"));
copyFileSync(coreWasm, resolve(outWasmRoot, "vide-core.wasm"));
copyFileSync(resolve(workspaceRoot, "crates", "vide-lsp-wasm", "js", "vide-lsp.adapter.js"), resolve(outWasmRoot, "vide-lsp.js"));

console.log(`Built Vide WASM adapter into ${outWasmRoot}`);

function assertFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} not found at ${path}`);
  }
}

function findEmscriptenRoot() {
  if (process.env.EMSDK) {
    const root = resolve(process.env.EMSDK, "upstream", "emscripten");
    if (hasEmscriptenTool(root, "emcc")) {
      return root;
    }
  }

  const emcc = findToolOnPath("emcc");
  if (emcc) {
    return dirname(emcc);
  }

  throw new Error(
    "Emscripten SDK is not configured. Activate emsdk_env first or set EMSDK before running npm run build:wasm.",
  );
}

function emscriptenTool(name) {
  if (!hasEmscriptenTool(emscriptenRoot, name)) {
    throw new Error(`Emscripten tool '${name}' not found under ${emscriptenRoot}.`);
  }
  return emscriptenToolPath(emscriptenRoot, name);
}

function hasEmscriptenTool(root, name) {
  return emscriptenToolPath(root, name) !== undefined;
}

function emscriptenToolPath(root, name) {
  const candidates =
    process.platform === "win32" ? [`${name}.bat`, `${name}.cmd`, `${name}.exe`, name] : [name];
  for (const candidate of candidates) {
    const path = resolve(root, candidate);
    if (existsSync(path)) {
      return path;
    }
  }
  return undefined;
}

function findToolOnPath(name) {
  const pathEntries = (process.env.PATH ?? "").split(delimiter).filter(Boolean);
  for (const pathEntry of pathEntries) {
    const path = emscriptenToolPath(pathEntry, name);
    if (path) {
      return path;
    }
  }
  return undefined;
}
