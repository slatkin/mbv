# Orchestration handoff — remove-legacy-keyboard-endpoint

State as of 2026-08-28. Supersedes the earlier U1–U16 plan in this file;
that plan was abandoned after U1 (scope too large per unit, two workers
killed before committing).

## Current state

- **HEAD:** `d47381f` — plan artifacts revised after exploration.
- **Working tree:** clean.
- **Worker model:** `openrouter/deepseek/deepseek-v4-flash-0731` for all
  delegated agents (per user directive for this change).
- **One serial writer.** Many units touch shared files (`shell.rs`, `msg.rs`,
  `key_policy.rs`, `input*.rs`); no parallel writes.
- **One commit per unit.** No amend, no push. Verify with `git show`.

## What the exploration settled (folded into design.md, commit `d47381f`)

1. **Task 2.4 deleted.** Skip-intro/next-up is *already* a focused modal
   (`PlaybackPromptComponent` + `application.active()` in `sync_playback_prompt`).
   Focus is the blocking mechanism; no `SubClause::HasAttrValue` mirror needed.
   `ATTR_SKIP_INTRO_PROMPT_VISIBLE`/`ATTR_NEXT_UP_PROMPT_VISIBLE` are dead (init
   `false`, never read as guards) and are deleted with task 2.3.

2. **Two factual errors corrected in proposal.md + design.md.** The
   `handle_key_with_home_context` call sites in `shell_home.rs` are
   `#[cfg(test)]`-only, not production bypasses.

3. **Decision 6 recorded (Space/Escape).** Space and Escape are global
   playback keys (double-tap → Stop / TogglePlayPause). Legacy `CONTEXT_STACK`
   ran playback's double-tap *above* the focused leaf; TuiRealm's fan-out fires
   the leaf and the Playback subscription independently, so a leaf claiming
   Space/Escape double-acts on the second press. Resolution: **one global
   handler owns the 300ms double-tap and dispatches the existing typed
   first-press leaf request by focus** (`BrowserBack`/`TvBack` for browse
   `go_back`, `AudiobookshelfBookIntent::Play`/`PodcastEpisodeIntent::FocusOrPlay`
   for Audiobookshelf select) on the first press, the playback command on the
   second. Leaves stop claiming Space/Escape. No per-screen playback-timer
   mirror (the reason the migration exists). This is the one genuinely new
   mechanism; everything else is mechanical family conversion.

4. **Dot-key (`.`) is NOT a decision.** The component-resolves-and-emits-target
   pattern is already established (`browser.rs` → `BrowserContextMenu { item }`,
   `music_workspace.rs` → `MusicTrackContextMenu`). Home/TV/ABS follow it; the
   Home CW target is resolved by the HomeComponent from Model-owned
   `home_content`, same site as today, emitted rather than threaded.

## Why the first pass never finished

The proposal's "Why" (D15's circular "unreferenced before wiring" gate) is the
documented reason. Exploration this session found **no deeper technical wall**
beyond the two things now settled (Space/Escape ownership, skip-intro modal
already-focused). The dot-key and the five `push_*_content` re-projections are
mechanical, inventoried in the U1 handoff §5. The remaining work is bounded.

## Remaining work (tasks.md, current)

Mechanical family conversion (no new decisions):

- **2.2** UiRoot globals (overlay/force-clear/refresh/Panel-mode/tab/quit + F1 + Alt-key path).
- **2.3** Playback chords + **Decision 6 global Space/Esc owner** + delete dead prompt attrs.
- **2.5** Dot-key for Home/TV/ABS (follow existing browser/music pattern).
- **3.1** Overlays/dialogs (Confirm, DaemonLost, RemoteReanchor, ContextMenu, PlaybackPrompt) → typed intents.
- **3.2** Settings/Feeds/SavePlaylist/forms → typed intents (incl. cursor-carrying `*SettingsKey`).
- **4.1** Queue ownership.
- **5.1–5.3** Leaves stop emitting `GlobalViewKey` (Home/Browser, TV/Music, ABS).
- **6.1** Delete legacy endpoint + raw `*Key` variants + `typed_key.rs` + blanket `push_*` → targeted.
- **6.2** Architecture gate rejects raw `KeyEvent` payloads.
- **7.1–7.2** Final gates.

## Per-unit dispatch checklist (orchestrator)

The earlier U1–U16 unit split was too coarse (U2 and U3 both killed for scope).
Dispatch the **smallest independent family** per worker; do not bundle
unrelated concerns. Each worker:

1. Starts from clean HEAD; reports the base SHA.
2. Plain-prose `task` naming ONE task row + the exact files.
3. `openrouter/deepseek/deepseek-v4-flash-0731`, `async:true`.
4. Reuses existing shell effect methods; does not duplicate effect logic.
5. Preserves exact behavior (every key that does something keeps doing it;
   every swallowed key stays swallowed).
6. Runs `rtk cargo check -p mbv`, focused `rtk cargo nextest run -p mbv` on
   touched surfaces, `rtk cargo fmt` before committing. Known baseline:
   3 pre-existing failures in `tests_conformance_matrix.rs` (227/291/339) +
   `music_resize` — ignore, do not chase.
7. One new commit, no `Co-Authored-By` trailer.
8. Reports new SHA + `git show --stat HEAD` + any unsure behavior (flag, don't guess).

## Gates deferred to final unit (7.1/7.2)

- `rtk cargo fmt`, `rtk cargo check -p mbv`, `rtk cargo nextest run -p mbv`,
  `rtk cargo clippy --workspace --all-targets`, `rtk ast-grep scan`,
  `rtk make check-code-file-lines`.
- Verify zero production matches for `CONTEXT_STACK`, `handle_legacy_key`,
  `handle_key_with_home_context`, `GlobalViewKey`, raw shell `*Key` request,
  `to_crossterm_key_event`.
- ast-grep baseline is 69 diagnostics from the prior campaign; only flag NEW.