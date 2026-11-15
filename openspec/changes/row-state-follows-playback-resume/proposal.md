# Proposal

## Why

A row's resume state (Played, Active with a progress percentage, or Ordinary) does not always tell the user what Play will do. The rule that derives it has several copies, and the copies disagree by screen, not by provider. Home, the Queue, and the Emby lists let a position win over played. The Podcasts and Feeds lists, in both Clients, let played win. No list uses the 1 percent gate that Playback resume uses, so a row can show Active 0% for an item that starts from the beginning. Feeds show a live bug: the context-menu Mark Played keeps the position, so the same entry shows Played in the Feeds list and Active on Home and in the Queue (#1005, split from #996).

## What Changes

- One rule decides a row's resume state for every screen and both Clients: **a row is Active exactly when Play would resume from a saved position.** The rule follows each provider's Resume source:
  - Emby video and Feed entry: a position that passes the Playback resume gate wins over played.
  - Audiobookshelf: a finished item is Played, whatever its position, because the Audiobookshelf server starts a finished item from zero. An unfinished item with a position that passes the gate is Active.
  - Music and Twitch: always Ordinary (unchanged).
- The Active test is the Playback resume gate (`should_resume`), not "position above zero".
- **Visible:** Podcasts and Feeds lists show Active for a played item that Play would resume. Every list shows Played or Ordinary, not Active 0%, for a position below the gate. Queue and Home rows of a finished Audiobookshelf item show Played, not Active near 100%.
- **Visible:** the Feeds context-menu Mark Played (one row, a multi-selection, or the Home or Queue toggle) stores position zero, as playback completion does. Mark Unplayed keeps the position.
- **Visible:** after a Feeds Mark Played or Mark Unplayed, the loaded Feeds list rows show the new state at once. Today they change only on the next refresh. Queue slots of feed entries stay out of scope.
- **Visible, one time:** stored feed-entry rows that are played and keep a position get position zero when the store loads, and the store is saved once.
- The row date gutter (provider timestamp, short date, no gutter when the date cannot be formatted) has one helper, which every list uses.
- `GuiHomeRowState` and its `from_progress` are removed. The GUI Library projections hold `MediaSemanticState`, and the bridge maps it to the existing wire DTO. The wire shape does not change.
- ADR 0036 gets an amendment: row resume state, progress percentage, and row dates are Service facts with one shared rule per provider, and the two Library panels differ only in which facts they show and where.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `playback-resume`: adds the requirement that a row shows Active exactly when Play would resume, per Resume source, on every screen and in both Clients.
- `canonical-media-lists`: the meaning of `Active` changes from "stored resume progress" to "Play would resume from a saved position".
- `feed-subscriptions`: the played indication applies to a played entry that Play would not resume. A played entry that Play would resume shows its resume state.
- `feed-queue-item`: Mark Played on a feed entry stores position zero, and stored played rows never keep a position.

## Impact

- `crates/mbv-ui-model`: `MediaSemanticState` gets per-provider constructors (Emby, Feed entry, Audiobookshelf) over one gated body. `from_queue_item` dispatches to them. A row-date helper joins `ui_util` or `home_latest`.
- `crates/mbv-components`: the Podcasts (`podcast_content.rs`) and Feeds (`feeds_content.rs`) rows call the shared constructors. Every date-gutter site calls the helper.
- `src/app/gui_library`: the Podcasts, Feeds, Home, Shows, and Emby projections call the shared constructors and the date helper. `GuiHomeRowState` goes from `src/app.rs` and `src/lib.rs` exports.
- `crates/mbv-gui/src/bridge/frame/library`: `GuiHomeRowStateFrame` maps from `MediaSemanticState`.
- `crates/mbv-feed`: `FeedEntryStore` keeps played rows at position zero (on `put` and on `load`).
- `src/app/dispatch/context_menu/actions.rs` and `src/app/dispatch/feeds/`: the Feeds mark applies the stored state to the loaded `feed_tab` entries.
- `src/app/state/construct.rs`: the store load saves once when it corrected rows.
- `docs/adr/0036-gui-library-panel-owns-its-interaction-state.md`, `docs/invariants/` (a new invariant), `CONTEXT.md` (*Playback resume*).
- No dependency changes. No ctrl protocol change. No change to the Player's resume decision.
