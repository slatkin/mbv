## Releasing

1. Bump `version` in `Cargo.toml`
2. `cargo build` to update `Cargo.lock`
3. Commit: `Release X.Y.Z: <one-line summary>`
4. Push — a GitHub Action automatically updates the PKGBUILD sha256
