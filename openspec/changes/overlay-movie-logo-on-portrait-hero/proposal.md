## Why

Movie logos are useful presentation artwork, but mbv currently treats `Logo` only as a fallback candidate for the hero's base image. A Movie whose Wide Hero uses landscape artwork can instead use an available transparent logo as a restrained decoration over its fanart while preserving the portable single-image rendering path.

## What Changes

- Parse Emby's declared Movie logo image tag so logo availability is known before fetching.
- For a Movie using the Landscape Wide Hero header, fetch its logo independently and alpha-composite it into the top-left of the final landscape bitmap.
- Show the landscape art without waiting for the optional logo; rebuild the hero image when a cold logo fetch subsequently succeeds.
- Leave Portrait Wide Hero artwork, Narrow inline heroes, non-Movie heroes, and Movies without usable logos unchanged.
- Keep playback badges and progress overlays outside this change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-panel`: Define the optional Movie-logo decoration of landscape artwork in the Wide Hero header and its fallback behavior.

## Impact

- Affects Emby item image-tag parsing, hero artwork content/projection, image fetching and cache identity, and Wide Hero protocol construction.
- Uses the existing `image` and `ratatui-image` dependencies; no new dependency or terminal-specific graphics path is introduced.
- Adds one optional Emby image request for the selected eligible Movie, satisfied from the existing image disk cache when warm.
