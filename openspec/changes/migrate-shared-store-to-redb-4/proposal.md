## Why

mbv's redb 2.6.3 dependency creates `shared.mbvd` with redb file format v2, while redb 4.x rejects that format with `UpgradeRequired(2)`. PR #486 therefore cannot be merged safely until existing stores are upgraded to file format v3 without compromising shared state or playback availability (GitHub issue #490).

The upgrade has modest immediate value for mbv: its narrow, serialized use of basic table operations does not exercise most of redb 4.x's important correctness fixes or concurrent-read improvements. This change therefore prioritizes a low-risk format transition on redb 2.6.3 and does not treat the major-version gap itself as sufficient reason to upgrade before planned shared-store work such as issue #472.

## What Changes

- Upgrade existing file-format-v2 `shared.mbvd` stores to file format v3 while mbv still uses redb 2.6.3.
- Create new shared stores in file format v3 so subsequent shared-store changes are written in the future-compatible format.
- Make migration idempotent and preserve the original store when opening or migration fails.
- Keep playback operational and shared-data hosting unavailable for that run when the store cannot be migrated or opened.
- Verify that every known application-owned table, stored record, and logical revision survives migration and restart, including tables introduced by issue #472 if it lands first.
- Defer the redb 4.x dependency bump to a later change after the format-v3 transition is established and direct-upgrade compatibility has been reconsidered.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `shared-mbv-state`: Require compatible shared-store format creation, safe migration of existing stores, failure isolation, and preservation of logical shared state across the redb upgrade.

## Impact

- Affects shared-store initialization and its persisted `shared.mbvd` file in `mbv-core`.
- Keeps the workspace on redb 2.6.3 for this change; a later proposal may upgrade the runtime to redb 4.x.
- Requires persisted-file migration and restart verification around the existing shared-store tests.
- Does not change the shared-data wire protocol, capabilities, document schema, or logical revision semantics.
- Does not block issue #472: that change can use redb 2.6.3 and the same format-v3 database.
- Tracks GitHub issue #490 and establishes the prerequisite for any safe replacement for PR #486's dependency-only bump.
