# Remove obsolete bespoke media-list loops

## Why

After the canonical media-list foundation and all four destination slices are
accepted, the old cross-family row loops, painters, cursor/scroll geometry, and
layout fields are dead migration scaffolding. Keeping them invites a second
rendering vocabulary and makes future fixes diverge. Two things are excluded
from deletion: per-surface `*HitRegion` row-hit geometry (issue #638,
`restore-mouse-support`, owns migrating it and lands after this slice) and the canonical controls'
own component-owned viewport geometry (retained).

## Scope and dependency boundary

The canonical foundation must land first. Home/Feeds, Music/Audiobookshelf,
and Queue are independent sibling destination slices; cleanup waits for all
four accepted slices (foundation plus those three siblings). Their accepted
SHAs are recorded when implementation issues are created, not pinned here.

This cleanup is a distinct deletion/documentation-only PR targeting
`feat/migrate-tui-to-tuirealm`. That branch is its destination, not a boundary
from which this change is based. No destination slice, visual correction, or
umbrella artifact is included.

## What changes

Delete only obsolete cross-family bespoke media-list painters, row loops, and
cursor/scroll geometry, and `AppLayout::main` left/hero/selector/wide-family
fields after staged zero-reference proof. Do NOT delete per-surface `*HitRegion`
row enums or hit-test geometry (`restore-mouse-support` #638 owns migrating those
onto the canonical controls and lands after this slice) and do NOT delete the
canonical controls' component-owned viewport geometry.
Reconcile ADR 0022, the interactive-surface ledger, `CONTEXT.md`, and frontend
guidance with final `WideMediaList`/`InlineMediaBrowser` ownership and
terminology.

## Invariants

Preserve the non-hero two-column carve-out, Queue fixed-row-only presentation,
and the distinction between the Feeds Service/tab and an Emby homevideos feed
view. Preserve every per-surface `*HitRegion` enum and bespoke hit-test path for
`restore-mouse-support` (#638). Any destination or visual defect routes to its
owning slice; this cleanup makes no visual corrections. Umbrella 4.x/5.x final gates remain in the
umbrella. Cleanup, UI test/documentation updates, automated gates, review, and
acceptance form one uninterrupted slice; test and documentation references are
tracked separately from production references.
