# Tasks

## 1. Compositor foundation (mbv-images)

- [x] 1.1 Add `ab_glyph` and an embedded static Lexend Deca weight + OFL text under
  `crates/mbv-images`; verify `cargo check -p mbv-images` passes and the licence file is included
  in the crate's assets.
- [x] 1.2 Implement `title_overlay.rs`: `covers(text)` and `compose_title_overlay` (scrim
  gradients, one row top / top+bottom, shrink-to-floor then ellipsis, cell-relative size). Contract
  owned: the compositor's pixel output. Verify with unit tests: one-part draws top only and leaves
  the bottom pixels untouched; two-part draws both; long text stays within width and ends with
  ellipsis; `covers` is false for a CJK char; same title at two box sizes yields the same text
  height in cell rows. No geometry-coordinate assertions on TUI panes.

## 2. Cache and projection (shell)

- [x] 2.1 Add `NowPlayingTitleSite` to `mbv-ui-model` and the shell's single computation of it
  (design D4) as two steps - overlay eligible, and site = eligible + painted-fact recorded beside
  `record_card_size` - keyed by the suffix-independent item identity. Verify with a table-driven
  unit test (`#[case]` only where outcomes differ): active+protocol+painted -> Artwork; paused ->
  Artwork; eligible but not yet painted, halfblock, visualizer, `visual_slot_shown()` false (idle /
  hidden / zero slot), no images, uncovered glyph, idle card on cursor art -> Header; a
  dim-backdrop suffix flip leaves the site unchanged for both an Emby and an Audiobookshelf key.
- [x] 2.2 Add `ensure_title_overlay_protocol` next to `ensure_hero_cover_protocol`: measure the
  box from the base protocol, build the composed variant as its own cache entry
  `{item-identity}:t:{cols}x{rows}:{hash}` (design D3: the card cache key with any protocol suffix
  stripped), recompose only on identity/box/title change; a suffix change only re-encodes through
  `cached_image_protocol_mut`. Verify with a test (hermetic, in the style of
  `arriving_logo_rebuilds_base_only_protocol_once`) that playback ticks build zero protocols, a
  track change builds one, and a suffix flip builds a protocol without recomposing.
- [x] 2.3 Make `render_queue_playback_slot` / `card.rs` paint the overlay variant when the overlay
  is eligible (not when the site is `Artwork`, which needs the paint to have happened) and the
  plain entry otherwise; verify the plain `:P` entry bitmap is unchanged after an
  overlay is built (regression guard for the shared-key flash).
- [ ] 2.4 Measure compose + encode time on a real large cover at track change (manual check, per
  the repo's no-live-tests rule); record the number in the PR. Only if it hitches the UI, move the
  compose into the resize worker as a follow-up task.

## 3. Header and spec sync

- [ ] 3.1 Feed the site into `QueuePlaybackPanel` and paint `Now Playing` after the state
  icon (aqua play, yellow pause) when the site is `Artwork`, the existing title otherwise
  (`render_header_title` / `HeaderTitle`); the label paints in `PLAYBACK_CONTEXT_FG` (yellow)
  after the icon. Verify
  with panel painter tests: Artwork -> icon + `Now Playing` in those colours; Header ->
  existing title rows (extend the existing header tests, do not duplicate); Idle unchanged.
- [ ] 3.2 Add `CONTEXT.md` terms if new domain words were introduced (title site, overlay); verify
  by reading `CONTEXT.md`'s Avoid list first and checking no collision.
- [ ] 3.3 Manual visual pass on Kitty and Sixel (and halfblock for the fallback): tune scrim
  opacity/height and Lexend weight (design Open Question); verify one-part, two-part, long-title,
  and a CJK title (falls back to the header).
- [ ] 3.4 Sync the delta specs into `openspec/specs/` (the queue-playback-panel header and queue-column layout
  requirement and the new `queue-artwork-title-overlay` capability); verify
  `openspec validate queue-art-title-overlay --strict` passes. The new capability's `## Purpose`
  moves into the main spec's Purpose section on sync.
- [ ] 3.5 Archive the change (`/opsx:archive`) once 3.4 is done; verify it no longer appears in
  `openspec list` and the main specs carry the new requirements.
