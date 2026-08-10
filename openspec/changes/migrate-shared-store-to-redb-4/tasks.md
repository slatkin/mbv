## 1. Format target and migration boundary

- [ ] 1.1 Pin the existing workspace dependency to redb 2.6.3 and create new `shared.mbvd` databases explicitly in file format v3 without adding redb 4.x or a second redb major.
- [ ] 1.2 Refactor shared-store initialization to distinguish a supported format-v2 upgrade from corruption, unsupported-format, permission, and other open failures without changing the existing shared-hosting/playback failure boundary.
- [ ] 1.3 Define one migration registry for every application-owned redb table present when implementation begins, including issue #472's feed-entry-state table if that change has landed.

## 2. Safe physical migration

- [ ] 2.1 Implement a private migration helper that removes only orphaned migration staging files, snapshots every registered table, and copies the closed format-v2 database to a unique sibling staging path with the existing restrictive permission policy.
- [ ] 2.2 Upgrade only the staged copy through redb 2.6.3, close it, reopen it as format v3 with redb 2.6.3, and verify complete logical equality for every registered table before any replacement.
- [ ] 2.3 Atomically replace `shared.mbvd` with the validated same-directory staged file, reopen it through the normal redb 2.6.3 path, and make cleanup/retry behavior deterministic for disk-full, interruption, and validation failures while preserving the original before replacement.
- [ ] 2.4 Ensure a missing store is created directly as format v3 and that a current format-v3 store bypasses the migration path.

## 3. Compatibility verification

- [ ] 3.1 Extend the existing shared-store test boundary with one persisted-file scenario that creates format v2 through redb 2.6.3, migrates it, proves preservation of representative records from every registered table and logical revisions, then proves a second open is idempotent.
- [ ] 3.2 Add one focused failure/recovery scenario that proves a failed or abandoned staged migration leaves the authoritative legacy store recoverable, does not promote the staging file, and succeeds on a later retry.
- [ ] 3.3 Verify the daemon startup caller still logs shared storage as unavailable and continues playback initialization when migration returns an error; strengthen an existing boundary test only if direct inspection cannot establish that behavior.

## 4. Final checks and coordination

- [ ] 4.1 Run `cargo check -p mbv-core`, `cargo test -p mbv-core`, `cargo clippy --workspace --all-targets`, and `make check-code-file-lines`, all through the repository-required `rtk` prefix.
- [ ] 4.2 Confirm the lockfile remains on the single pinned redb 2.6.3 dependency with no unrelated changes, reference issue #490 from the implementation PR, and close or supersede PR #486 without merging its dependency-only bump.
