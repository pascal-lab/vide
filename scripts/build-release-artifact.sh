#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: $0 <rust-target> <vsix-target>}"
vsix_target="${2:?usage: $0 <rust-target> <vsix-target>}"

case "$target" in
  x86_64-unknown-linux-gnu)
    build_target="x86_64-unknown-linux-gnu.2.17"
    cargo_cmd=(cargo zigbuild --release --target "$build_target" -p vide --bin vide)
    binary="target/$target/release/vide"
    ;;
  aarch64-unknown-linux-gnu)
    build_target="aarch64-unknown-linux-gnu.2.17"
    cargo_cmd=(cargo zigbuild --release --target "$build_target" -p vide --bin vide)
    binary="target/$target/release/vide"
    ;;
  x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
    build_target="$target"
    cargo_cmd=(cargo zigbuild --release --target "$build_target" -p vide --bin vide)
    binary="target/$target/release/vide"
    ;;
  aarch64-apple-darwin)
    build_target="$target"
    cargo_cmd=(cargo build --release --target "$build_target" -p vide --bin vide)
    binary="target/$target/release/vide"
    ;;
  x86_64-pc-windows-msvc)
    build_target="$target"
    cargo_cmd=(cargo build --release --target "$build_target" -p vide --bin vide)
    binary="target/$target/release/vide.exe"
    ;;
  *)
    echo "unsupported release target: $target" >&2
    exit 2
    ;;
esac

echo "::group::Build $target"
printf 'command:'
printf ' %q' "${cargo_cmd[@]}"
printf '\n'
"${cargo_cmd[@]}"
echo "::endgroup::"

test -s "$binary"
"$binary" --version || true

case "$target" in
  x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu)
    echo "::group::Verify GNU Linux ABI contract"
    readelf -d "$binary" | grep 'Shared library' || true
    readelf -V "$binary" | grep -E 'Name: (GLIBC|GLIBCXX|CXXABI)_' | sort -uV || true
    if readelf -d "$binary" | grep -E 'Shared library: \[(libstdc\+\+|libgcc_s)\.so'; then
      echo "$target dynamically depends on the C++ runtime." >&2
      exit 1
    fi
    if readelf -V "$binary" | grep -E 'Name: GLIBC_2\.([2-9][0-9]|1[89])\b|Name: GLIBCXX_|Name: CXXABI_'; then
      echo "$target violates the glibc 2.17 / no dynamic C++ ABI contract." >&2
      exit 1
    fi
    echo "::endgroup::"
    ;;
  x86_64-unknown-linux-musl|aarch64-unknown-linux-musl)
    echo "::group::Verify musl static ABI contract"
    file "$binary"
    readelf -l "$binary" | grep 'Requesting program interpreter' && {
      echo "$target has a program interpreter; expected fully static." >&2
      exit 1
    } || true
    if readelf -d "$binary" 2>/dev/null | grep NEEDED; then
      echo "$target has dynamic dependencies; expected fully static." >&2
      exit 1
    fi
    if readelf -V "$binary" 2>/dev/null | grep -E 'Name: (GLIBC|GLIBCXX|CXXABI)_'; then
      echo "$target has glibc/C++ ABI version dependencies; expected fully static musl." >&2
      exit 1
    fi
    echo "::endgroup::"
    ;;
  aarch64-apple-darwin)
    echo "::group::Verify macOS deployment metadata"
    otool -l "$binary" | grep -A5 -E 'LC_BUILD_VERSION|LC_VERSION_MIN_MACOSX'
    echo "::endgroup::"
    ;;
esac

mkdir -p target/distrib
archive_stem="vide-$target"

case "$target" in
  x86_64-pc-windows-msvc)
    archive="$archive_stem.zip"
    rm -rf "$archive_stem" "$archive" "$archive.sha256"
    mkdir -p "$archive_stem"
    cp LICENSE "$archive_stem/LICENSE"
    cp README.md "$archive_stem/README.md"
    cp "$binary" "$archive_stem/vide.exe"
    (cd "$archive_stem" && 7z a "../$archive" LICENSE README.md vide.exe)
    ;;
  *)
    archive="$archive_stem.tar.xz"
    rm -rf "$archive_stem" "$archive" "$archive.sha256"
    mkdir -p "$archive_stem"
    install -m 0644 LICENSE "$archive_stem/LICENSE"
    install -m 0644 README.md "$archive_stem/README.md"
    install -m 0755 "$binary" "$archive_stem/vide"
    tar cJf "$archive" "$archive_stem"
    ;;
esac

sha256sum "$archive" > "$archive.sha256"
mv "$archive" "$archive.sha256" target/distrib/

# Stage the same binary for the VS Code package. VSIX targets intentionally use
# VS Code's platform names, not Rust target triples.
server_dir="editors/vscode/server/$vsix_target"
mkdir -p "$server_dir"
if [[ "$vsix_target" == win32-* ]]; then
  install -m 0755 "$binary" "$server_dir/vide.exe"
else
  install -m 0755 "$binary" "$server_dir/vide"
fi
