# Remove legacy `--connect-daemon` / `daemon_client_endpoint` startup path (#230)

**ADR:** none needed — pure removal, no new decision to record.

**Prerequisite for:** #223 (library-scoped daemon routing). #223's design-item-8 (precedence between `--connect-daemon` and the new `daemon_routes."*"` wildcard) is moot once this lands — there is nothing left to conflict with.

**Not #222 follow-on work:** this was originally scoped as a "Task 6" appended to #222's plan (`2026-07-17-daemon-connect-lifecycle.md`) during a post-merge grilling-session review, but #222 is closed/merged (PR #228). New work motivated by and blocking #223 belongs in its own issue and plan, not bolted onto a closed issue's plan doc — hence #230 and this file. The Task 6 section has been removed from `2026-07-17-daemon-connect-lifecycle.md`.

## Rationale

The user has never used `--connect-daemon` in practice; keeping it around only to preserve an unused startup path is not worth the maintenance surface.

## Files

- Modify: `src/main.rs` — remove `connect_daemon_arg` (lines 41-55), `run_remote_app` (lines 15-39, only caller is the block being removed), the `cli_daemon_endpoint` binding and its match (lines 171-177), the `explicit_daemon_endpoint` construction (lines 235-245) and the `if let Some(endpoint) = explicit_daemon_endpoint { ... }` branch (lines 259-278), the `--connect-daemon` line in `print_usage` (lines 187-189), the stale "No `--connect-daemon` can be present here" comment near the stay-alive inferior-argv block (~line 336), and the two unit tests `connect_daemon_arg_accepts_split_and_equals_forms` / `connect_daemon_arg_requires_value`.
- Modify: `crates/mbv-core/src/config.rs` — remove `Config.daemon_client_endpoint` (struct field, `Default` impl, and its `[daemon.client] endpoint` TOML parsing in `parse_config`). A `[daemon.client]` section left over in an existing user's `config.toml` becomes inert (silently ignored, like any other unrecognized TOML key already is in this parser) — no migration warning, consistent with how this parser already treats unknown keys.
- Modify: `README.md`, `CONTEXT.md` — remove usage/glossary mentions of `--connect-daemon` / `daemon_client_endpoint`.
- Modify (light touch, not a rewrite): `docs/adr/0006-single-instance-flock-and-socket-detection.md` line 26 and #222's ADR (`docs/adr/0010-lazy-daemon-route-connect-lifecycle.md`) lines 7-8 — both mention `--connect-daemon` only in passing, as context for other decisions, not as their subject. Edited in place to drop the stale reference rather than superseded with a new ADR — the flag's presence in those docs was itself incidental noise from when the plans were drafted, not a decision worth preserving a historical trail for.

## Interfaces

- Removes: `main.rs::connect_daemon_arg`, `main.rs::run_remote_app`, `Config.daemon_client_endpoint`. No new public interface — this is a pure removal.
- After this change, `remote_player::RemotePlayer::connect_endpoint` and `remote_player::DaemonEndpoint::parse` remain in use (Sessions-panel `connect_direct_endpoint`, and #222's `connect_daemon_route_endpoint`) — only the startup-flag call site goes away.

## Steps

- [ ] **Step 1: Remove the CLI/config code path** in `src/main.rs` and `crates/mbv-core/src/config.rs` as scoped above.
- [ ] **Step 2: Update docs** (`README.md`, `CONTEXT.md`, the two ADR passing-mentions) to drop the removed flag.
- [ ] **Step 3: Run `cargo build --workspace` and `cargo test --workspace`** — expect a clean build (no dead-code warnings from the removal) and all remaining tests passing; the two flag-parser tests are deleted, not left failing.
- [ ] **Step 4: Run `cargo clippy --workspace --all-targets -- -D warnings`** — expect clean.
- [ ] **Step 5: Commit**

```bash
git add src/main.rs crates/mbv-core/src/config.rs README.md CONTEXT.md docs/adr/0006-single-instance-flock-and-socket-detection.md docs/adr/0010-lazy-daemon-route-connect-lifecycle.md
git commit -m "remove: legacy --connect-daemon / daemon_client_endpoint startup path (#230)"
```
