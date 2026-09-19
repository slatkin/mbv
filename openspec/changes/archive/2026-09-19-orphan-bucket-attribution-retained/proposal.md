# Proposal: orphan-bucket-attribution-retained

## Why

The archived `fix-music-artist-resolution-batching` change recorded design D3
as dropping an orphan bucket when no in-hand album `Path` matched it. That rule
predates commit `f4e9c44e`, which established whole-level artist fills while the
`albums` argument may contain only one browse page.

A bucket key is normally the track `ParentId`, which is 1:1 with an album.
When an album appears on a later page, its key is indistinguishable from a
nested-disc orphan until the full level is known. Warm-up passes an empty album
list, so dropping all unmatched keys would make startup artist resolution
return nothing and force every grouped view through the browse-time upgrade.

## What Changes

Supersede design D3's drop rule with the established retained-unknown-key
behaviour:

- keep `Path`-matched orphan tracks attributed to the longest matching album
  path;
- retain tracks whose orphan key has no matching path under that original key;
- allow the resulting inert cache row to be ignored by later album lookups;
- preserve whole-level coverage for later-page albums and startup warm-up with
  an empty album list.

No new capability or UI behaviour is introduced; this record documents the
accepted implementation correction and intentionally uses `skip_specs: true`.

## Impact

- `src/app/image_fetch.rs` — retain unmatched orphan buckets and update the
  D3 rationale comment and regression tests.
