## Why

Tab selection is a bare `library_tab: usize` whose meaning lives only in a comment (`app_struct.rs:148`: "0 = Home, 1..=libs.len() = library index"). That convention is re-derived by hand at ~90 sites — 43× `library_tab - 1`, 29× `library_tab > 0`, 19× `library_tab == 0`, 3× `checked_sub(1)`. Adding the Feeds tab (#471) means introducing a tab that is *not* an Emby library; with a bare index, nothing stops any of those sites from reading the feeds slot as a library. Typing the selection now makes that class of bug unrepresentable and gives #471 a compiler-enforced seam instead of an audit.

## What Changes

- Replace the `library_tab: usize` field with a typed `TabSelection` enum: `Home` and `Library(usize)` (index into `libs`). **No `Feeds` variant yet** — #471 adds it, and doing so will make every match exhaustively account for it.
- Route all "which Emby library is this tab?" access through a single accessor, `library_index(&self) -> Option<usize>`, so the mapping exists in exactly one place. The 43 `library_tab - 1` / `checked_sub(1)` sites and the `> 0` guards call it instead of doing arithmetic.
- Preserve the existing selection API shape (`set_library_tab`, `library_tab_next`, `library_tab_prev`, `library_tab_count`) and the render/mouse position mapping; only their internals change.
- **No behavior change.** Tab selection, cycling, rendering, and routing behave identically. This is a pure refactor — hence `skip_specs`.

## Capabilities

This change opts out of specs (`skip_specs: true`): it is a pure refactor with no spec-level behavior change.

## Impact

- **Field + accessors:** `library_tab` in `app_struct.rs`; setter/cycling in `cw_library_tab_actions.rs`; the pending→committed apply in `render/mod.rs`.
- **~90 read sites** across `src/app/` (actions, input, render, mouse, tests) migrate from index arithmetic to `TabSelection` methods.
- **Enables #471** (feeds-tab): adds the `Feeds` variant, at which point non-exhaustive matches become compile errors — the intended safety.
- **Depends on nothing** — independent of #470/#471; can land while #470 is in flight.
