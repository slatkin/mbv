## Releasing

Preferred path:

```sh
scripts/release.sh X.Y.Z "one-line summary"
```

What the script does:

1. Runs the CHECKIN steps (`cargo test` and `cargo clippy`)
2. Bumps `version` in `Cargo.toml`
3. Runs `cargo build` to update `Cargo.lock`
4. Commits `Release vX.Y.Z: <one-line summary>`
5. Pushes `main`
6. Creates `vX.Y.Z` and pushes the tag

Important:

- Pushing `main` alone does not create a GitHub release.
- The GitHub release, asset upload, PKGBUILD update on `main`, and AUR push happen on the tag-triggered workflow for `vX.Y.Z`.
- Re-index jCodemunch only if that MCP tool is available in the current environment.

Manual equivalent:

1. Follow CHECKIN.md steps (test, clippy)
2. If available, re-index jCodemunch: call `index_folder` on the repo root
3. Bump `version` in `Cargo.toml`
4. `cargo build`
5. `git add Cargo.toml Cargo.lock`
6. `git commit -m "Release vX.Y.Z: <one-line summary>"`
7. `git push origin main`
8. `git tag vX.Y.Z && git push origin vX.Y.Z`
