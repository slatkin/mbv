## Why

The non-Wide Inline hero replaces the selected media row with a second, variable-height presentation that is harder to navigate and duplicates the Wide Hero pane's detail vocabulary. Non-Wide browsing should keep the standard media list stable and reveal the same rich Hero content on demand without preventing independent Queue use.

## What Changes

- Replace Inline hero and selected-row replacement in every non-Wide geometry with the standard fixed-row media-list presentation.
- Add a Library Hero overlay, opened with Enter on a hero-bearing browser item and confined to 85% of the Library pane.
- Reuse the Wide Hero header, overview, and Workspace content inside the overlay; focus a present Workspace list and keep it open after child activation.
- Keep the Queue visible and fully operable while the Library Hero overlay remains open but unfocused.
- Dismiss the overlay with Esc while Library holds focus, by clicking the dimmed Library backdrop without click-through, or by changing Library destination.
- Preserve normal leaf activation behind a second Enter and leave Inline Search behavior unchanged.
- Remove the constituent-list selection modal and the Inline media-list presentation because the Library Hero overlay supersedes both.

## Capabilities

### New Capabilities

- `library-hero-overlay`: Defines the Library-local detail overlay, its geometry, focus, dismissal, activation, and coexistence with Queue.

### Modified Capabilities

- `canonical-media-lists`: Removes the Inline presentation and makes the standard fixed-row presentation canonical in both Wide and non-Wide browser geometry.
- `library-panel`: Replaces Narrow Inline hero composition with the standard list plus the Library Hero overlay while reusing Wide Hero content.
- `inline-hero-selection-modal`: Removes the superseded constituent-list modal behavior.

## Impact

- Affects Library panel composition, media-list presentation ownership, destination-local Hero and Workspace interaction, keyboard routing, mouse hit geometry, image projection, and overlay-focused integration tests under `src/app/`.
- Deletes Inline hero painting and the `InlineMediaBrowser` adapter after callers move to the shared fixed-row list.
- Replaces the existing constituent selection popup path for TV, Music, Audiobookshelf podcasts, and Audiobookshelf books; no Service, Player, queue-authority, persistence, or external protocol behavior changes.
- Requires updating the presentation terminology in `CONTEXT.md` and synchronizing the affected durable specifications when the change is archived.
