## Context

See `proposal.md` for motivation and `specs/library-panel/spec.md` for behavior.

Correction 2026-09-15: the first implementation pass decorated the Portrait hero and was reverted unshipped (commit `85c30b0b`); the requirement is Landscape-only. Portrait/landscape wording below is normative for the redo.

The shared hero artwork policy chooses a shape from Emby's declared image tags before fetching. For an Emby source, `project_hero_image` reserves the selected base-image chain, and `ensure_hero_cover_protocol` cover-fits the decoded source to the final artwork box before constructing one `ratatui-image` protocol. `CachedImage` retains the decoded source and remembers only the current hero box. This already gives the correct portable compositing seam: after cover-fit and before protocol construction.

`EmbyImageTags` does not currently retain `ImageTags.Logo`. `Logo` instead appears as the last fallback in base-image chains, so it cannot yet be addressed as independently declared decoration. Image fetches are deduplicated and disk-cached by cache key, with a bounded decoded-image LRU for successful entries.

## Goals / Non-Goals

**Goals:**

- Preserve one terminal image placement for the complete decorated fanart.
- Keep optional-logo loading off the base image's critical path.
- Make protocol reuse sensitive to the final box and the particular loaded logo.
- Keep eligibility derived from provider-neutral hero content and shared presentation state.

**Non-Goals:**

- No generic overlay framework or caller-selected Hero variant.
- No raw Kitty placement, z-index, or terminal-specific cleanup lifecycle.
- No SVG runtime, embedded play asset, playback badge, progress ring, or live animation.
- No decoration of list cards or inline artwork.

## Decisions

### D1 — A Logo is an optional semantic artwork decoration, not a base-image fallback

Add Logo availability to `EmbyImageTags` from `ImageTags.Logo`. The Emby Movie hero producer carries an optional independently keyed Logo source beside its base artwork only when the item is a Movie and a Logo is declared. The shared Library panel derives eligibility from Movie content, `ArtworkShape::Landscape`, and Wide geometry; the Movies destination does not select a new presentation arm.

Remove `Logo` from Movie base-artwork fallback chains touched by this change. A separately authored transparent title treatment is not a valid full poster fallback, and keeping it in both roles would permit the same image to become both base and decoration after a partial failure.

*Alternative:* infer availability by requesting `/Images/Logo` for every landscape Movie. Rejected because known absence is normal provider metadata and should not create network failures.

### D2 — Fetch the Logo independently through the existing image pipeline

Give the Logo its own cache key containing the Movie identity, Logo kind, and declared Logo tag. Reuse the existing bounded fetch queue, disk-byte cache, decode path, completion channel, and decoded-image LRU rather than adding a second worker system. Queue this optional fetch only while projecting an eligible Wide Landscape Movie hero; Portrait, Narrow, placeholder, and non-Movie paths do not request it.

A ready base remains `HeroImageState::Ready` whether the Logo is absent, pending, failed, or decoded. Logo completion merely causes the normal subsequent projection to reconsider the protocol input.

*Alternative:* fetch base and Logo in one worker and return a pair. Rejected because it would delay the poster, weaken independent cache reuse, and duplicate failure handling.

### D3 — Composite at the final pixel size before protocol construction

First cover-fit the landscape base to the exact hero-box pixel dimensions. If the eligible Logo is decoded, convert the final base and resized Logo to compatible RGBA images and use `image::imageops::overlay`, which alpha-blends at a pixel coordinate. Construct `ThreadProtocol` from that single composited bitmap, preserving the same behavior for Kitty, Sixel, iTerm2, and halfblocks.

The Logo preserves aspect ratio and fits within 60% of the landscape width and 20% of its height. Its top and left inset are 5% of the corresponding landscape dimension, rounded and clamped so the resized Logo remains within the landscape art. Do not add a backing plate, shadow, or synthetic opacity; provider-authored transparency is authoritative.

*Alternative:* render two `StatefulImage` widgets. Rejected because overlapping Ratatui buffer cells do not provide reliable image compositing. Raw Kitty z-index placements are rejected because they bypass the shared protocol abstraction and do not cover Sixel, iTerm2, or halfblocks.

### D4 — Protocol validity includes the applied Logo identity

Extend the hero protocol's remembered derivation from only `cover_box` to `cover_box` plus the optional applied Logo cache key. While the Logo is pending or failed, the base-only protocol is valid with no applied Logo. When a successful cold Logo fetch appears, the desired applied key changes and exactly that hero protocol is rebuilt. Resizes and protocol-suffix changes continue through existing invalidation paths.

Retain original base and Logo images independently. Do not persist a third composited artifact: recreating one bounded bitmap on selection or resize is cheaper and simpler than another disk-cache lifecycle.

### D5 — Decoration remains presentation-only

The Hero facts carry semantic image sources and decoded readiness; neither the screen producer nor the destination computes rectangles or composites pixels. The existing shell image projection resolves fetch/cache state, and the existing hero image protocol builder owns pixel composition. No pointer geometry changes because Wide Movie heroes remain read-only and the decoration has no action.

## Risks / Trade-offs

- **[A cold eligible selection encodes the base image twice]** → Show the base immediately, then rebuild once when the Logo succeeds; responsiveness is preferred over waiting for optional decoration.
- **[Large transparent Logo sources add decode memory and bandwidth]** → Reuse the bounded fetch queue, 10 MiB response limit, disk cache, and decoded-image LRU; request the existing bounded Emby Logo representation rather than the original.
- **[Provider logos can have unusual whitespace or contrast]** → Preserve provider pixels and apply only contain-sizing and inset; avoid heuristics, shadows, and background plates in the first version.
- **[Existing cache entries do not key base images by Emby image tag]** → Key the new Logo entry by its declared tag so changed decoration cannot reuse stale bytes; changing the broader base-image cache policy is outside this change.
- **[A Logo completion must invalidate a base-derived protocol stored under another key]** → Compare the desired applied Logo key during every normal hero projection instead of attempting cross-entry mutation in the completion drain.

## Migration Plan

Client-only additive parsing and presentation behavior; no persisted application state or wire compatibility changes. Existing serialized items without a Logo tag deserialize to an empty value and remain undecorated. Rollback removes the decoration field and compositing path; independently cached Logo bytes are harmless and expire under the existing image-cache policy.
