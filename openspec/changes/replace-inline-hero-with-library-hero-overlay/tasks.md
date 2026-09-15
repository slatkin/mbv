## 1. Protect and Extract Shared Hero Composition

- [x] 1.1 Consolidate the existing focused Wide Hero buffer coverage around header, overview, Workspace, and image-box output before extraction; verify the targeted Wide Hero and Library-panel render tests pass unchanged.
- [x] 1.2 Extract the Hero header, Main content box, Workspace placement, selector, and image geometry from the Wide skeleton into one shared Library-panel composition path; verify existing Wide Hero buffer tests and image-area integration assertions remain green.

## 2. Add the Library Hero Overlay

- [x] 2.1 Add Library-panel-local open/dismiss state and a centered 85-percent Library-pane arrangement with frame, Library-only dimming, and `Esc` hint; verify arrangement tests prove containment at Narrow, Mini, constrained-height, and both-panels geometry without absolute coordinate assertions.
- [x] 2.2 Render the shared Hero composition inside the overlay and return its current image and Workspace geometry through the Library panel's existing paint result; verify focused buffer tests cover leaf and Workspace Heroes without changing Wide output.
- [x] 2.3 Gate covered browser geometry while the overlay is open, retain overlay/backdrop hit geometry from the latest frame, and consume outside-Library dismissal without click-through; verify focused Interactive Component tests cover stale geometry and dismissal without underlying selection mutation.

## 3. Route Focus and Activation

- [x] 3.1 Make non-Wide Enter and browser-row double-click open the selected item's Library Hero overlay while leaving Inline Search unchanged; verify mounted tick and latest-frame mouse tests cover one parent with a Workspace, one leaf, and one Inline Search result.
- [x] 3.2 Give a present Workspace its existing destination-owned focus on open, preserve its cursor and scroll across Queue focus, and keep the overlay open after Enter or double-click activation; verify mounted Music or TV integration coverage proves child activation and focus restoration through the shell sync pass.
- [x] 3.3 Implement leaf second-Enter and in-overlay double-click activation, Library-focused Esc dismissal, destination-change dismissal, and missing-parent dismissal; verify focused component tests prove browser target/scroll preservation and that Queue-focused Esc does not close the overlay.
- [x] 3.4 Preserve independent Queue keyboard and mouse operation while the overlay remains visible and unfocused; verify mounted routing and mouse integration tests cover Queue focus, one Queue action, return to Library, and retained overlay Workspace state.

## 4. Replace the Inline Presentation

- [x] 4.1 Change `MediaListCarrier` and the Library panel so the fixed-row presentation remains active across Wide and non-Wide geometry, preserving the canonical owner and clamping its viewport without presentation-to-presentation state transfer; verify media-list and Library-panel tests cover stable target and viewport behavior in both directions.
- [x] 4.2 Remove Inline selected-row admission, painting, paint policy, detail geometry, and `InlineMediaBrowser` after all callers use fixed rows; verify code search finds no remaining Inline presentation or selected-row-replacement production path and targeted canonical media-list tests pass.

## 5. Migrate Destination Workspaces

- [x] 5.1 Route TV and grouped Music non-Wide parent opening and child activation through their existing Wide Workspace owners in the Library Hero overlay; verify their focused component and mounted tick integration tests cover loading/ready content and stable child targets.
- [x] 5.2 Route Audiobookshelf Podcast and Book non-Wide parent opening, filters/selectors, provider completion, and child activation through their existing Wide Workspace owners; verify focused component and mounted tick integration tests cover loading, empty, ready, and stable-target refresh without real Services or sleeps.
- [x] 5.3 Confirm Home, Movies, generic Emby catalogs, home videos, and Feeds use fixed browser rows and leaf Hero overlays without introducing destination painters; verify representative mounted and buffer tests cover one no-Workspace Hero and one destination change.

## 6. Remove the Superseded Modal and Finish Terminology

- [x] 6.1 Remove constituent `SelectionModal` construction and dispatch from TV, Music, Audiobookshelf Podcast, and Book, then delete selection-modal component/render/type code only if no unrelated caller remains; verify structural search and compilation show no obsolete constituent-modal path while shared modal-frame users remain intact.
- [x] 6.2 Update `CONTEXT.md` to define Library Hero overlay and remove Inline hero, selected-row replacement, InlineMediaBrowser, Presentation, and Carrier claims that no longer describe the implementation; verify terminology agrees with the change specs and introduces no collision with application popup or Sidebar terms.

## 7. Verification

- [x] 7.1 Run `cargo fmt`, `cargo check -p mbv`, and the narrowest affected component, render, and mounted tick integration suites with `cargo nextest run -p mbv`; verify Wide, Narrow, Mini, short-height, keyboard, mouse, and Queue-coexistence coverage is green without live externals, sleeps, snapshots, or whole-frame assertions.
- [x] 7.2 Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run -p mbv --no-fail-fast`; verify the workspace is clean and all mbv tests pass before review.
