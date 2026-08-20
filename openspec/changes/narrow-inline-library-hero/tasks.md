## 1. Establish the shared narrow layout contract

- [x] 1.1 Trace the current narrow arrangement dispatch, one-column row geometry, hero sizing, scroll state, and common mouse hit-target production across all library screens.
- [x] 1.2 Define the shared inline-hero flow segment and row-map representation so cursor indices remain media-item based while hero-owned rows remain inert.
- [x] 1.3 Replace the narrow hero-on-top dispatch for library browse screens with the shared inline-hero presentation without changing the centralized breakpoint or wide arrangement selection.

## 2. Render the inline hero

- [x] 2.1 Reuse each library's existing hero content declaration, artwork/loading path, metadata, and selection styling in the shared narrow inline block.
- [x] 2.2 Render the active media row and its variable-height hero as one scrolling flow segment, suppressing the hero only when the minimum active content cannot fit.
- [x] 2.3 Preserve ordinary one-column row content and selection markers for inactive rows, including transitions when the cursor changes.

## 3. Preserve interaction and scrolling behavior

- [x] 3.1 Update keyboard cursor movement and visible-row accounting so the active row and inline hero remain addressable together while scrolling.
- [x] 3.2 Produce common mouse hit targets for media rows only; keep hero-only space inert and preserve existing row activation behavior.
- [x] 3.3 Verify loading, empty, filtered, grouped, and long-metadata library states do not create invalid row maps or inaccessible active rows.

## 4. Verification and cleanup

- [x] 4.1 Add focused coverage for narrow inline placement, cursor changes, variable hero height, insufficient height, scrolling, and mouse hit targets across representative library kinds.
- [x] 4.2 Add regression coverage confirming wide hero-on-top and hero-on-left layouts, breakpoint behavior, and non-library narrow screens are unchanged.
- [x] 4.3 Run focused tests, formatting, clippy, and file-size checks; inspect narrow and wide render captures for all supported library screens.
