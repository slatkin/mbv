## Context

See proposal.md — Why. The constraints that shape the approach:

- The TUI is single-instance (ADR 0006 flock + control-socket resolution), so a local state
  file has exactly one writer. `mbv-core` has no second writer of user state.
- Four of the five things the shared store holds already have a local file that the client
  writes **first**, then pushes to the daemon (`persist_shared_document` mirrors before it
  sends). Only feed-entry state reads and writes exclusively through the daemon.
- Existing local persistence (`config_state.rs`) already provides the atomic
  temp-file/rename pattern, the "missing or invalid → absent state, log, continue"
  behavior, and the drop-based test guards `TestStateDirGuard` / `TestTempDir` that #687
  recommends but `shared_store.rs` bypassed.
- The daemon never wrote feed-entry state; those writes are TUI-side (playback lifecycle and
  cast status). Removal does not move that responsibility.

## Goals / Non-Goals

**Goals:**

- Delete the shared-data capability completely, with no compatibility shim and no dead
  configuration surface left behind.
- Keep feed-entry state durable and keep its three user-visible behaviors — All / Played /
  Unplayed filter, the "Watched" marker, resume-on-play — while dropping cross-machine
  roaming.
- Land the feed-state migration before the deletion, so the deletion is mechanical and the
  Feeds tab is never broken mid-change.
- Remove the `redb` dependency and its leaking test harness.

**Non-Goals:**

- No replacement roaming carrier. Emby-side storage, a file-sync integration, or a shared
  state directory are all explicitly out of scope.
- Not a general fix for test tmpdir leaks. #687's remaining sites (`applog.rs`, the stale
  -dir sweep, adopting a guard-based pattern repo-wide) stay with #687.
- No changes to the four local document schemas, to ctrl, or to the Emby session
  `supported_commands` entry used for ctrl discovery (`mbv-direct-tcp-port`).
- Not making UI state roam. Session continuity remains deliberately unoffered.

## Decisions

**1. Feed state becomes a single local JSON file with flat keyed rows.**
`state_dir()/feed_entry_state.json` holds an array of `{user_id, feed_id, entry_guid,
position_ticks, played}`. A row is replaced by key on write; a prefix scan filters by
`(user_id, feed_id)`. Alternatives rejected: a key-concatenated map (needs separator
escaping — `feed_id` is a URL and `guid` is arbitrary); nested user→feed→guid maps (exact
prefix behavior, but three levels of nesting for no gain); keeping `redb` as a local database
(smallest deletion, but retains the dependency, the store, and the harness that caused #687,
and needs a migration reader for `shared.mbvd` to be useful).

**2. The whole map is loaded once at startup and rewritten on change.**
The store is a small in-memory value owned by `App`, persisted whole. Ceiling: a full rewrite
per write, O(rows). Acceptable because rows are bounded by feed entries the user has actually
played, and writes happen on playback lifecycle events, not per frame.

**3. Reuse the existing local-state machinery rather than adding a new persistence layer.**
`FeedEntryState` moves beside the other persisted state types; the path joins
`config_paths.rs`; load/save join `config_state.rs` and use its atomic write. The three
feed-state functions (`hydrate_feed_entry_state`, `hydrate_feed_entries_for_subscription`,
`write_feed_entry_state`) keep their names and call shapes, so their call sites change only in
what they call underneath. They relocate from `shared_sync.rs` into `feed_tab_actions.rs`,
which is already the feed-specific home and stays well under the file-size bar.

**4. New tests use the existing drop guards, not raw `temp_dir()` joins.**
`TestTempDir` (added in 47926ae0) and `TestStateDirGuard` remove their directories on drop,
including on panic, which is the pattern #687 settled on. Repeating the old pattern here would
be a self-inflicted regression.

**5. Residue is abandoned, not migrated.**
`shared.mbvd` and `roaming_settings.json` are left on disk unread; nothing is imported. A
leftover `[shared_data]` section is removed from `config.toml` on the next settings save,
since `save_config_settings_at` preserves the rest of the document and would otherwise carry
the dead section forever. Alternative rejected: a one-time adoption of
`roaming_settings.json` into `config.toml` — it adds a migration path and a startup branch to
rescue a value whose only distinctive property (silently overriding explicit local config)
is precisely what this change removes.

**6. Two ordered task groups, one change.**
Group 1 introduces the local store and switches the TUI onto it while the shared modules
still exist and go unused. Group 2 deletes. Rationale: group 1 is behavior-preserving and
provable with a round-trip test, so if group 2 lands in a later session the Feeds tab is
already correct and every remaining step is subtraction.

**7. The capability is removed outright, not deprecated in place.**
Alternatives rejected: keeping the modules behind a disabled flag (keeps the maintenance
surface and the redb dependency, and leaves the spec describing a facility that does not
exist); keeping the client code against third-party hosters (no such hosters, and the
protocol is undocumented and purpose-built).

**8. Three main-spec edits are made directly rather than through a delta.** OpenSpec deltas
cannot carry a capability `Purpose`, and `interactive-component-framework`'s stale enumeration is
prose rather than behavior. Task 1.5 therefore edits `openspec/specs/feed-entry-state/spec.md`,
`feed-subscriptions`, and `interactive-component-framework` directly. Validation is unaffected:
deltas are matched against requirement and scenario names, not Purpose text, and task 1.5
re-runs strict validation after the edits to confirm that rather than assume it.

## Risks / Trade-offs

- **[Existing users lose feed watched markers once.]** No migration reader is written. →
  Accepted: rows are re-derived from play activity, and the alternative is a `shared.mbvd`
  reader that outlives the feature it exists to retire.
- **[A machine whose only copy of `library_routes` lived in the shared store loses them.]** →
  Verified: the F2 handler saves config (`shell_overlays_menus.rs:696` → `render::save_route_config`
  → `config::save_config_settings`) *before* persisting to shared data (`:700`), so a machine
  that edited routes keeps them in `config.toml`. A machine that only inherited them reverts to
  its own `config.toml` — the intended single-machine behavior, stated in the commit body
  (task 2.4). The inbound direction (shared records overwriting local files in
  `apply_shared_snapshot`) is the roaming behavior this change removes, not a competing local
  authority.
- **[Resume could write a zero position over a queued slot's position.]** `action.rs:425-440`
  hydrates an entry and then calls `apply_progress`. → With the local store the hydrated value is
  the stored position, matching today's shared-connected behavior; task 1.3 pins the queue-slot
  case with a test rather than leaving it to reasoning.
- **[A name-based sweep deletes unrelated state.]** `daemon_core.rs:492`'s `SharedQueueState` is
  ctrl snapshot state (queue, source, observed active slot), not shared data. → Called out
  explicitly in task 2.3.
- **[`redb` removal cascades.]** Verified: `mbv-core` is the only workspace package depending on
  it (no other package's dependencies reference it in `Cargo.lock`) and no other source file
  imports it. → Dropping it from the two manifests is sufficient.
- **[Deleting 3.6k lines leaves orphan references.]** → Compiler is the oracle (no
  `#[allow(dead_code)]` escapes), plus a final `rg` sweep for
  `shared_data|shared_client|SharedClient|shared_store|roaming|shared-mbv-state` before the
  change is called done.
- **[Delta format keeps three legacy scenario names in `feed-subscriptions` and one in
  `feed-entry-state`.]** A MODIFIED requirement replaces its whole block, and the validator
  refuses to drop a scenario the current spec still has, so "No playback state is remembered",
  "Shared entry state is unavailable", "State changes on another machine", and "Feed entry
  commit fails" are retained by name with bodies restated for local state. → Deliberate:
  dropping those labels would need the requirement removed and re-added, which the validator
  rejects ("Requirement present in both ADDED and REMOVED"). Reconsider only if the delta
  format gains scenario-level removal.
- **[`shared_store.rs`'s #687 fix (47926ae0) becomes dead code.]** The in-memory `redb`
  backend that stopped the tmpfs leak is deleted with the file. → None needed: the fix is
  committed and the worktree is clean, so deleting the file cannot silently swallow in-flight
  work; nothing else depends on it.
- **[`config_save` deleting a user's section.]** → It only removes `[shared_data]`, whose keys
  are inert after this change; matching how empty `[library_routes]` and removed
  `[audiobookshelf]` sections are already handled.
- **[Old client against a new daemon.]** → It finds no `mbv-shared-data-tcp-port` in the Emby
  session and behaves as a client with no shared-data endpoint: local state, no error. No
  protocol version bump is needed because the ctrl capability list never covered shared data.

## Migration Plan

1. Group 1 lands on its own: local store + TUI switch + its tests. Feeds behavior is
   unchanged; the shared modules are simply no longer called for feed state.
2. Group 2 deletes the transport, hosting, configuration, CLI action, glyph, and specs.
3. Rollback is `git revert` of either group. Because `shared.mbvd` is never deleted, reverting
   group 2 restores the previous behavior with its data intact; reverting group 1 alone leaves
   feed state written to the local file and read by the old code path only if group 2 has not
   landed.
4. After merge: update #687 (primary leaking site removed; remaining sites still open).

## Open Questions

- Whether any future need for cross-machine state should be met by a file-sync-friendly
  state directory rather than an in-process carrier. Deferred; it does not affect this
  design, the specs, or the task breakdown.
- Whether a daemon that owns feed playback with no attached client should record feed-entry
  state. It does not today, and this change preserves that gap rather than widening it.
