## Releasing

1. Follow CHECKIN.md steps
2. Bump `version` in `Cargo.toml`
3. `cargo build` to update `Cargo.lock`
4. Commit: `Release X.Y.Z: <one-line summary>`
5. Push — a GitHub Action automatically updates the PKGBUILD sha256
6. Monitor CI for green
