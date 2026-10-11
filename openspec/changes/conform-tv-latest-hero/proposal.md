# Proposal

## Why

TV Latest does not show a hero in Wide geometry, but every other Latest list does. TV already paints through the shared Library panel. The difference comes from three TV-only rules: a second breakpoint source that inspects row types, an owner hero gate on that wrong breakpoint bit, and a panel branch that names TV. The archived `rework-tv-library-pills` change limited its scope with "No episode hero or workspace outside mini view", and the specs then stated that limit as required behavior. No product decision made TV different.

TV Upcoming rows are Emby premiere notices (`LocationType: Virtual`, no `Id`, no media sources). They are not media items. The code still carries them as `EmbyItem` episodes and tells them apart with `id.is_empty()` checks, so each new path can treat a notice as playable media by mistake (the 2026-10-06 mini-view Upcoming hero pop-up is one example).

## What Changes

- In Wide geometry, TV Latest shows the selected episode's hero in the shared Hero pane, the same as Movies Latest.
- TV gets Wide or Narrow from terminal geometry only, the same as every other destination. The "are the root rows all Series?" check leaves the breakpoint. Series mode against flat mode becomes a content-mode fact that the TV owner already holds.
- The Library panel stops naming TV. A destination says that its current rows are not media, and then the panel paints no Hero pane.
- **BREAKING** TV Upcoming rows become premiere notices, a separate non-media type. A notice has no hero in any geometry and cannot play or enqueue. Activation opens its series. The rule "an Upcoming row with an episode id plays" is removed.
- **BREAKING** In non-Wide geometry, Enter or double-click on a row of any Latest list plays it at once. No Library Hero overlay opens. This changes Movies, Home Videos, other Emby libraries, and Feeds Latest, which open the overlay first today. TV and podcast Latest already play at once.
- **BREAKING** The TV-only mini-view auto-opening episode hero is removed. Mini view works the same as on every other destination.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tv-library-content-modes`: Upcoming rows are premiere notices, not episodes. Latest shows the shared Wide hero. The mini-view hero and the "Upcoming row with an id plays" rule are removed.
- `destination-latest-modes`: the TV exception sentence is removed. All Latest rows play on Enter and double-click in non-Wide geometry, and show their hero in the Wide Hero pane.
- `library-hero-overlay`: Latest rows are not hero-entry rows in non-Wide geometry.
- `library-panel`: a list of non-media rows has no Hero pane in Wide geometry.
- `tv-tree-browsing`: the flat-mode requirement stops naming "their existing Hero rules" and stops calling Upcoming rows episodes.

## Impact

- `crates/mbv-components`: `tv_content` (owner hero, breakpoint push, Upcoming rows and activation), `library_panel` (hero-pane gate, mini-view auto-open removal, trait methods), `emby_library_content` and `feeds_content` (Latest Enter and double-click policy).
- `src/app`: `state/projection/tv_wide.rs` (breakpoint), `shell/tv_workspace.rs`, `shell/run.rs`, `input/browse_dispatch.rs`, `shell/overlays/menus.rs`, the Upcoming snapshot and activation paths in `dispatch/library`.
- `crates/mbv-emby` and `crates/mbv-emby-model`: `get_upcoming` returns premiere notices, not `EmbyItem`.
- `crates/mbv-ui-msg`: the Upcoming activation request carries a series reference, not an `EmbyItem`.
- `CONTEXT.md`: a new *Upcoming premiere* term.
