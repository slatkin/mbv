## Context

The proposal describes the compatibility break between mbv's existing redb 2.6.3 stores and redb 4.x. `shared.mbvd` is a single daemon-owned redb file opened only when shared-data hosting is enabled. Opening it is part of shared hosting startup, and failure already leaves playback operational.

redb's crate major version and file-format version are independent. Existing mbv releases create file-format-v2 stores. redb 2.6.3 can migrate those stores and can create or operate file-format-v3 stores; redb 4.x accepts format v3 but rejects format v2.

The newer runtime offers upstream maintenance, memory, correctness, and performance improvements, but mbv does not use most of the affected advanced APIs and serializes a small shared-state workload through one worker. The immediate user-visible benefit is therefore limited, while combining the physical migration with a major runtime upgrade raises recovery complexity. Issue #472 may safely add its feed-state table on redb 2.6.3 because crate version 2.6 can operate the target file format v3.

## Goals / Non-Goals

**Goals:**

- Transition existing and new stores to file format v3 while remaining on redb 2.6.3.
- Preserve all committed shared documents and their application-level revisions.
- Preserve the original database bytes until a migrated replacement has been upgraded and validated.
- Keep migration local to shared-store initialization and retain the existing playback failure boundary.
- Make startup recovery deterministic after interruption at any migration step.
- Preserve every known application-owned table, including a feed-state table if issue #472 lands before this change.

**Non-Goals:**

- Change shared document schemas, protocol capabilities, authentication, or revision rules.
- Add a general database backup, import, repair, or downgrade facility.
- Migrate logical queue or settings document shapes.
- Make shared-data hosting available while physical migration is running.
- Upgrade the normal storage runtime to redb 4.x.

## Decisions

### Perform the format transition entirely on redb 2.6.3

The migration release will retain redb 2.6.3 as its sole redb dependency. Existing format-v2 stores will be migrated with `Database::upgrade()`, and new stores will be created explicitly in file format v3 through the redb 2.6 builder.

This separates the risky persisted-format transition from a low-urgency runtime change and avoids shipping two redb majors. A simultaneous dual-version runtime was rejected because it increases binary and maintenance complexity without a compelling immediate benefit. Reimplementing redb's conversion was rejected because it would duplicate storage-engine internals.

### Let redb 2.6 identify and upgrade the legacy format

Shared-store initialization will open an existing database with redb 2.6.3 and determine whether its supported upgrade operation is required. New databases will be created directly in format v3. Corruption, unsupported versions, permissions, and other I/O errors will continue through the existing shared-hosting failure path without mutation.

This keeps format detection inside redb's supported API, avoids parsing private file headers, and prevents a generic open failure from being mistaken for a migratable format.

### Upgrade a sibling copy and atomically replace the original

Migration will copy the closed original database to a uniquely named sibling staging file, run redb 2.6's supported format upgrade against that copy, close it, and validate it by reopening it as format v3 with redb 2.6. Only then will the staged file atomically replace `shared.mbvd` in the same directory. The staging file will use restrictive permissions and be removed best-effort after any pre-replacement failure.

Copy-then-replace is preferred over upgrading the live file in place because disk-full, interruption, or conversion errors before replacement leave the original bytes untouched. The same-directory requirement keeps replacement on one filesystem so rename is atomic. The daemon's existing single-owner startup model prevents concurrent database writers during migration.

Validation will use a registry of every application-owned table known to the build and compare each table's complete logical contents before and after migration. This includes `shared_documents` and, when present in the codebase, issue #472's feed-entry-state table. Byte-for-byte value equality proves preservation of encoded document revisions without introducing migration metadata.

### Recover deterministically from staging artifacts

The original path remains authoritative until atomic replacement. On startup, an orphaned staging file will never supersede an existing `shared.mbvd`; it will be removed before a fresh migration attempt. After replacement, the authoritative path is already a validated format-v3 database and ordinary startup is idempotent. No permanent automatic backup is introduced.

### Keep migration below protocol and playback boundaries

Migration occurs before the shared listener starts. It does not add a shared-data or ctrl capability because no peer can observe a partially available migration operation. Any migration failure is reported through the existing database-unavailable startup path: shared hosting stays off for that run while playback remains operational.

## Risks / Trade-offs

- **Migration temporarily needs approximately one database file of free space.** → Upgrade a copy, surface disk-space failure as shared hosting unavailable, and leave the original untouched for retry.
- **A process or machine can stop during migration.** → Treat only `shared.mbvd` as authoritative, use a same-directory atomic replacement, and discard orphaned staging files on restart.
- **Replacement could lose restrictive file permissions.** → Create the staging file with restrictive permissions and reapply the existing shared-store permission policy before replacement and after normal open.
- **A false-positive migration attempt could mutate unsupported data.** → Use only redb 2.6's supported upgrade decision and never treat a generic open error as migratable.
- **Physical success could conceal logical data loss.** → Compare every registered table's logical contents before replacement, then verify the same data after reopening format v3 and again in a restart test.
- **A later redb 4.x release could strand users who skip this migration release.** → Keep the runtime bump out of this change; its future proposal must explicitly decide direct-upgrade support rather than assume universal installation of this release.

## Migration Plan

1. Keep redb pinned to 2.6.3 and isolate physical-format handling in shared-store migration code.
2. Register every application-owned table that must survive physical migration and create new stores explicitly in file format v3.
3. For an existing format-v2 store, snapshot every registered table, copy the closed file to a sibling staging path, upgrade the copy with redb 2.6, and close it.
4. Reopen the staged format-v3 database with redb 2.6, compare every registered table with the snapshot, apply restrictive permissions, and close it.
5. Atomically replace `shared.mbvd`, reopen it through the normal redb 2.6.3 path, and start shared hosting.
6. Verify new-store creation, format-v2 migration, already-migrated restart, interrupted/stale staging recovery, and migration failure with playback isolation.
7. Close or supersede PR #486 without merging its dependency-only bump; reconsider redb 4.x in a later proposal after this transition and issue #472's shared-store work.

Rollback within redb 2.6 remains possible because the migrated file uses format v3, which redb 2.6 can create and open. Logical table schemas are unchanged. If migration fails before replacement, rollback uses the untouched format-v2 original.
