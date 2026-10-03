# Hide the queue playback header row when the title lives on the artwork — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers-optimized:subagent-driven-development (recommended) or superpowers-optimized:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The queue playback header row is hidden exactly while playback is active and the now-playing title is painted on the artwork; it remains in every other frame — idle, or any state where the header is the title's home.

**Architecture:** This refines the ask's "kitty/sixel is not the image protocol" using the projection that already exists: `title_site == Artwork` implies kitty/sixel artwork with the overlay actually painted, and when the protocol is kitty/sixel but the overlay isn't painted (images off, visualizer up, visual slot hidden, glyph failure) the projection already says `Header`, so the header — the title's only home — stays. That closes the one real hole in the literal rule with zero new machinery. Row reclaiming is not a requirement: it falls out of passing `header_height: 0` through the existing `queue_panel_geometry` seam.

**Tech Stack:** Rust, ratatui (TUI), existing `mbv` app crate conventions.

## Global Constraints

- This is an ad-hoc change: no OpenSpec change, no spec/ADR/invariant edits.

## Rule

The queue playback header row is hidden exactly while playback is active and the now-playing title is
painted on the artwork; it remains in every other frame — idle, or any state where the header is the
title's home.

Exact predicate:
```rust
header_hidden = status != NowPlayingStatus::Idle
    && transport.title_site == NowPlayingTitleSite::Artwork
```

## Scope

Files touched (5 + deletions in 1):
1. `crates/mbv-render/src/arrangements/chrome.rs` — add `pub header_visible: bool` to `ChromeGeometryInput`; in `queue_column_geometry`, pass `header_height: if input.header_visible { QUEUE_PLAYBACK_HEADER_ROWS } else { 0 }`.
2. `src/app/shell/draw.rs` and `src/app/state/projection/tv_wide.rs` — set `header_visible` in both `ChromeGeometryInput` constructors (visible = `!(status != Idle && title_site == Artwork)`).
3. `src/app/shell/chrome_panels.rs` — in `sync_queue_playback_panel` and `render_queue_playback_panel`, replace the two literal `QUEUE_PLAYBACK_HEADER_ROWS` slot-region offsets with the resolved rows (`0` when hidden) so the slot/transport sit at the top of the collapsed placement.
4. `crates/mbv-components/src/queue_playback_panel.rs` — in `view()`, skip the header paint branch when the predicate holds (component has `self.status` and `self.transport.title_site` locally; no new field).
5. `crates/mbv-render/src/components/chrome_player/title/header.rs` — delete the now-unreachable `artwork_brand_spans` brand-row arm, `HeaderTitle::host`/`host_is_remote`, and the panel's `set_header` host params (shell call site updated); forced by the repo's no-dead-code rule since their only consumer is that arm. **Keep** `NowPlayingTitleSite`, `resolve_title_site`, `render_playback_header` (idle row), and the `Header` title painting — all still live.

## Tasks & Verification

- T1 Geometry collapse (files 1–3). Verify: `cargo nextest run -p mbv-render` plus one unit test in `arrangements/queue.rs`: with `header_height: 0`, `panel_area.y` rises by `QUEUE_PLAYBACK_HEADER_ROWS` (the reclaim contract, owned by the arrangement layer).
- T2 Paint gate (file 4). Verify: extend `src/app/tests/tick_integration/queue_playback.rs` `hidden_visual_slot_collapses_and_restores_queue_geometry_at_both_breakpoints` (or a sibling) with two rows: playing + `title_site == Artwork` → playback height shrinks by 2 and the queue panel starts 2 rows higher; playing + images off (`title_site == Header`) → the 2 rows remain (the title's home — this row is the regression guard for the hole).
- T3 Dead-code deletion (file 5). Verify: `cargo clippy --workspace --all-targets -- -D warnings` clean; delete the brand-row tests that exercised `artwork_brand_spans`.
- T4 Sweep: `grep QUEUE_PLAYBACK_HEADER_ROWS` — remaining uses only in chrome.rs (definition + collapse), chrome_panels.rs resolved sites, and tests. `cargo fmt`.

## Non-goals

- No new visibility enum, policy layer, or protocol sniffing plumbed into `mbv-render`; the predicate is one `&&` over two existing projected facts.
- No generalized "zero reserved rows" work beyond this header's 2 rows; visual-slot/transport geometry untouched.
- No overlay-compose or transient re-expansion handling: when the artwork overlay arrives mid-track the header collapses on the next frame through the normal sync path — same seam as card load, nothing special.
- No change to the idle header row (`[mbv] … IDLE` stays), to `NowPlayingTitleSite`, to `queue_band`, or to marquee state.
- No OpenSpec change, no ADR, no invariant doc — this is an ad-hoc change per the user's own framing.

## The implementer must not

- Add any requirement the ask doesn't contain (the three invented ones from the discarded proposal are the anti-pattern: zero-reserved-rows-as-goal, no-transient-re-expansion, brand-row deletion as an end in itself — the brand arm dies only because it's unreachable, not as a goal).
- Duplicate the predicate in more places than the five sites above; if a sixth site appears, route it through the same facts, don't fork the rule.
- Touch `mbv-images`, the overlay pipeline, or the card projection to make this work — `title_site` already carries everything.

Size: ~60–90 production lines added/edited across 5 files, ~60 deleted in `header.rs`, 2–3 test rows.
