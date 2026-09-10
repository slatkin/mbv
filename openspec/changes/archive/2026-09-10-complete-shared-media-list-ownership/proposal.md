# Complete Shared Media-List Ownership

## Why

PR #686 repaired cursor, scroll, painting, and retained geometry for its named destination lists, but advisor review found that the result still fails the campaign's motivating architectural test: a future purely list-local behavior would require edits in multiple destination components instead of one shared media-list subsystem. This corrective change completes that original outcome without implementing multi-select or broadening into unrelated sidebars, modals, or search controls.

## What Changes

- Make each in-scope logical media-row flow use one shared list state and behavior owner across Wide, Inline, and preserved two-column Grid presentations.
- Route eligible row-local keyboard and normalized pointer actions through one provider-neutral delegation API while keeping the mounted destination as the sole event boundary and typed-intent translator.
- Bring the preserved non-hero two-column Emby catalog presentation onto a shared Grid adapter without changing its two-column arrangement policy.
- Remove destination-owned row cursor, scroll, selected-row mirrors, render-time reseeding, compatibility row maps, and caller-supplied point-resolution fallbacks from the in-scope media-row flows.
- Use stable opaque row targets across refresh, reorder, responsive presentation changes, and typed destination requests.
- Cover Queue; Home; generic Emby, Movies, and the Emby homevideos feed view; Grouped Music albums and tracks; TV series and episodes; Feeds entries; Audiobookshelf Podcast shows and episodes; and Audiobookshelf Books and chapter/audio-part rows.
- Preserve parent ownership of Service content, pane focus, sections/groups/filters/buckets/seasons/scope pills, detail workspaces, images, effects, persistence, and provider-specific intent translation.
- Add mechanical architecture enforcement and a disposable acceptance probe proving that a purely list-local transition and decoration can be added without destination production edits.
- Correct the completion claim from the archived `finish-canonical-media-list-ownership` umbrella through this additive change; the archive remains unchanged historical evidence.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `canonical-media-lists`: Extend canonical ownership from shared one-column mechanics to one shared state/behavior seam for every PR-scoped media-row flow, including Grid and provider workspace rows, with a one-place extensibility acceptance criterion.

## Impact

- Primary code: `src/app/components/media_list/`, the corresponding shared Render Components, and the destination Interactive Components listed above.
- Source-of-truth projections and typed requests that still carry row indexes or mirrored selection will change to stable opaque targets before their callers.
- Current canonical-list tests and TuiRealm tick integration tests will be consolidated around shared delegation for Wide, Inline, Grid, and workspace presentations.
- Architecture rules will reject new destination-local media-row state, movement, compatibility painters, and row hit maps.
- No protocol, persistence format, Service behavior, dependency, or actual multi-select feature changes.
