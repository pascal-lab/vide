# Upstream: rust-analyzer VFS

This crate is an owned fork of rust-analyzer's virtual file system, following the
same approach as tinymist-vfs: copy upstream code, adapt to vide, do not track
`ra_ap_*` crates.io packages.

## Sources

| Component | Upstream path |
|-----------|---------------|
| Core VFS / loader / FileSet | https://github.com/rust-lang/rust-analyzer/tree/master/crates/vfs |
| Notify loader backend | https://github.com/rust-lang/rust-analyzer/tree/master/crates/vfs-notify |

- **Pinned commit:** `5300ee266534f8c68065285de005759c58ac7883`
- **Upstream licenses:** Apache-2.0 OR MIT (rust-analyzer)
- **Vide modifications:** owned by PASCAL Research Group / vide contributors (MIT)

## Intentional fork deltas (vide)

- Map `paths` / `stdx` onto `utils` instead of rust-analyzer workspace crates
- Feature-gate OS notify backend (`notify-backend`); `dummy::DummyHandle` when disabled (wasm)
- No VFS hardlink / path-identity redirects (removed relative to prior vide Vfs)
- SV-oriented loader configuration is supplied by callers (extensions, exclude prefixes)
- FileSet keeps PathMatcher / source filters for workspace source roots

## Sync policy

Manual only. Prefer pulling structural bugfixes from rust-analyzer; do **not**
reintroduce watcher completeness state machines or generation-scoped readiness
coupled to OS notify.
