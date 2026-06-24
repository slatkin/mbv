## Releasing

1. Follow CHECKIN.md steps (test, clippy, CI green)
2. Re-index jCodemunch: call `index_folder` on the repo root via the jCodemunch MCP tool
3. Bump `version` in `Cargo.toml`
4. `cargo build` to update `Cargo.lock`
5. Commit: `Release X.Y.Z: <one-line summary>`
6. Push — a GitHub Action automatically updates the PKGBUILD sha256
7. `git tag vX.Y.Z && git push origin vX.Y.Z`
