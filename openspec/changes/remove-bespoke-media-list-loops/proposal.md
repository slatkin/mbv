# Remove obsolete bespoke media-list loops

## Why

After the canonical media-list foundation and all four destination slices are
accepted, the old cross-family row loops, painters, hit geometry, and layout
fields are dead migration scaffolding. Keeping them invites a second rendering
vocabulary and makes future fixes diverge.

## What changes

Delete only obsolete cross-family bespoke media-list loops, painters, scrolling/
selection/hit geometry, and `AppLayout::main` left/hero/selector/wide-family
fields whose consumers have reached zero references. Reconcile ADR 0022,
the interactive-surface ledger, `CONTEXT.md`, and frontend guidance with the
canonical ownership model.

The cleanup is strictly downstream of these independently reversible slices:
foundation → Home/Feeds → Music/Audiobookshelf → Queue. It is a separate PR
and rollback boundary from PR #606's feature branch; it does not implement any
destination slice.

## Invariants

Preserve the non-hero two-column carve-out, Queue fixed-row-only presentation,
and the distinction between the Feeds Service/tab and an Emby homevideos feed
view. No bespoke exception, provider/effect/authority change, or new list
framework is introduced. Visual acceptance requires live user verification at
60x20 and Wide 120x40/140x30 before UI test changes.
