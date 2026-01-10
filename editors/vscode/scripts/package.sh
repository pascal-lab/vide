#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
VSCODE_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd -- "${VSCODE_DIR}/../.." && pwd)"

BIN_NAME="${BIN_NAME:-vizsla}"
HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

host_platform_folder() {
  case "${HOST_OS}-${HOST_ARCH}" in
    Darwin-arm64) echo "darwin-arm64" ;;
    Darwin-x86_64) echo "darwin-x64" ;;
    Linux-x86_64) echo "linux-x64" ;;
    Linux-aarch64 | Linux-arm64) echo "linux-arm64" ;;
    MINGW*-x86_64 | MSYS*-x86_64 | CYGWIN*-x86_64) echo "win32-x64" ;;
    *)
      echo "unsupported host platform: ${HOST_OS}-${HOST_ARCH}" >&2
      exit 1
      ;;
  esac
}

TARGET_PLATFORM_FOLDER="${1:-$(host_platform_folder)}"
HOST_PLATFORM_FOLDER="$(host_platform_folder)"

if [[ ! -d "${VSCODE_DIR}/node_modules" ]]; then
  if [[ -f "${VSCODE_DIR}/package-lock.json" ]]; then
    (cd "${VSCODE_DIR}" && npm ci)
  else
    (cd "${VSCODE_DIR}" && npm install)
  fi
fi

SERVER_OUT_DIR="${VSCODE_DIR}/server/${TARGET_PLATFORM_FOLDER}"
mkdir -p "${SERVER_OUT_DIR}"

if [[ "${TARGET_PLATFORM_FOLDER}" == "win32-x64" ]]; then
  BIN_FILE="${BIN_NAME}.exe"
else
  BIN_FILE="${BIN_NAME}"
fi

if [[ "${TARGET_PLATFORM_FOLDER}" == "${HOST_PLATFORM_FOLDER}" ]]; then
  (cd "${REPO_ROOT}" && cargo build --release)
  cp -f "${REPO_ROOT}/target/release/${BIN_FILE}" "${SERVER_OUT_DIR}/${BIN_FILE}"
  if [[ "${TARGET_PLATFORM_FOLDER}" != "win32-x64" ]]; then
    chmod 755 "${SERVER_OUT_DIR}/${BIN_FILE}"
  fi
else
  if [[ ! -f "${SERVER_OUT_DIR}/${BIN_FILE}" ]]; then
    echo "missing bundled server binary: ${SERVER_OUT_DIR}/${BIN_FILE}" >&2
    echo "tip: build/copy the correct binary into that path, or run without an explicit platform argument." >&2
    exit 1
  fi
fi

VSIX_OUT="vizsla-vscode-${TARGET_PLATFORM_FOLDER}.vsix"
cd "${VSCODE_DIR}"

# vsce prompts when LICENSE is missing; auto-confirm for local packaging.
printf 'y\n' | ./node_modules/.bin/vsce package --out "${VSIX_OUT}" >/dev/null

echo "${VSCODE_DIR}/${VSIX_OUT}"
