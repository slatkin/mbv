## Why

**Tracking issue:** [GitHub issue #646](https://github.com/slatkin/mbv/issues/646). The issue owns the bug report and cause analysis; this OpenSpec change owns the fix and completion gates.

Commit `0cbd51b7` changed the painted Series artwork cache key to `{id}:ser:{types}` but left the shell-side prefetch, loading-state, and completion re-push sites on the old `{id}:ser_primary` key. Every Series image is therefore fetched twice, the loading placeholder is skipped (blank shows instead), the completion re-push never fires for the real keys, and the LRU/disk cache hold two entries per series. The follow-up `464a6eb9` (real `Thumb` URL arm) makes reverting unsafe: the two chains now fetch different bytes, so the shared-key "clobbering" the original commit feared is real.

## What Changes

- Introduce one shared Series artwork cache-key constructor used by every fetch, lookup, and completion-match site, so the key format cannot drift again.
- Point the TV Wide prefetch (`shell_tv_workspace.rs`) at the painted Thumb-first chain key, fetching the same type chain the painter requests.
- Point the TV Wide and narrow `image_loading` lookups (`shell_tv_workspace.rs`, `list_narrow.rs`) at the painted keys so the placeholder shows until the painted entry lands.
- Widen the `tv_image_changed` completion gate (`shell_run.rs`) to match the `ser:` key family so image completion triggers a targeted TV re-push.
- Update the TV workspace shell test fixture key to the painted key.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-list-hero`: the "Hero content is independent of placement" requirement's "Hero content remains consistent" scenario already promises consistent declared image, loading state, and cache behavior across placements — this change makes Series artwork actually honor it (one fetch per series/chain, placeholder until the painted entry arrives, re-push on completion).

## Impact

- Code: `src/app/images.rs` (key helper home), `src/app/render/components/home_hero_emby.rs` (paint site adopts helper), `src/app/render/components/detail_series_view.rs` (narrow key adopts helper), `src/app/shell_tv_workspace.rs` (prefetch + loading), `src/app/render/components/list_narrow.rs` (loading), `src/app/shell_run.rs` (completion gate), `src/app/shell_tv_workspace_tests.rs` (fixture).
- No API, config, protocol, or dependency changes. No migration: cache keys are in-memory/LRU plus best-effort disk bytes; stale disk entries under the old key expire naturally.
- Out of scope: the larger TV Wide 16:9 kitty payload (cause 2 in #646) — that is the accepted cost of the landscape artwork design, not a key bug.
