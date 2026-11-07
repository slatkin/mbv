# Proposal: add-tauri-gui-client

## Why

The TUI ties mbv's keyboard-first interaction model to the character-cell grid. A native window can keep the model and drop the grid. The `extract-client-core` change (PR #984) adds the toolkit-neutral Client core that a second Client kind needs. This change adds that second Client.

The GUI does not copy the TUI. Its target is the design canvas "mbv Svelte Player" (https://claude.ai/artifact/D8uLGS6NyCeHd6ed7PHgsr): three artboards for the expanded panel, the collapsed states, and the menus and drag. Where the TUI, an earlier decision of this change, or an earlier spec line disagrees with the canvas, the canvas wins. A GUI that copies the TUI first and changes later wastes the work.

An earlier attempt used egui and was dropped. egui (and iced) rasterize text themselves in grayscale, and on a 1x monitor that text is visibly softer than the rest of the desktop. A webview draws text through the system font stack (FreeType and fontconfig), so the GUI's text is rasterized like the user's other applications. Tauri gives a Rust process with a system webview, and the Rust side can link the Client core directly.

## What Changes

- Add a GUI Client: a separate `mbv-gui` program built with Tauri 2. Its window content is a Svelte 5 front end that uses shadcn-svelte components and Lucide icons. On Linux the webview is WebKitGTK.
- The GUI Client attaches to the Owner process over ctrl like every other Client. It never runs playback. Video still plays in mpv's own window.
- The Rust side of `mbv-gui` links the `mbv` library and runs the Client core: the shell `App`, `ClientCore::tick`, the one router, the core focus stack, and Surface state. The front end paints projections that the Rust side sends, and it sends neutral key and pointer events back. It holds no copy of interaction behaviour.
- The first GUI slice presents the player panel of the design canvas: the header with the playback target, the artwork, the title, the scrubber (chapters, hover preview), the transport, the track chips with their popovers, the queue (Length / Ends at toggle, filter, row menu, drag reorder), the footer line, and the collapse states (compact strip, queue folded, rail). Other surfaces are absent from the GUI until later changes port them. The TUI keeps every surface and keeps working unchanged.
- The panel layout follows the canvas: the front end SHALL pick the layout from the measured size of the panel container, the same measurement that it reports to the Client core in `LayoutReport`, so the painted layout, the list capacity, and the hit map follow one size. Text uses IBM Plex Sans and JetBrains Mono, bundled with the front end.
- `mbv-gui` is a tokio edge, because Tauri requires tokio. The Client core stays synchronous.
- Not in scope: Library, search, overlays other than those the canvas shows, `--pin` for the GUI, tray from the GUI, GUI-specific MPRIS work (the GUI process claims MPRIS through `App` construction, as a second TUI Client does), desktop entry, release packaging of `mbv-gui`, and removal of the TUI.

## Capabilities

### New Capabilities

- `gui-client`: the windowed Client: launch and Owner attachment, the surfaces it presents, its interaction contract (one visible focus, every action on a key, no hover-only affordance, scroll follows the cursor, motion never delays input), its pointer behaviour, and the boundary between its Rust side and its web front end.

### Modified Capabilities

- `local-daemon-thin-client`: a GUI Client presents its own window and the surfaces of the `gui-client` capability, while a TUI Client keeps the full terminal UI. This builds on the wording that `extract-client-core` introduces.

## Impact

- Depends on `extract-client-core` (PR #984): `mbv-surfaces`, the `mbv` library target, `GuiInput`, `GuiSurfaceLeaf`, `ClientCore::tick`, `ClientWaker`, the `owner_exe` override, `declare_gui_client`, and the shared breakpoint classes. Implementation starts after that PR merges.
- New crate: `crates/mbv-gui` (Tauri 2 binary). It is not in `default-members`, so the default build and test runs skip it.
- New front end: `crates/mbv-gui/ui/` (Svelte 5, Vite, TypeScript, Tailwind CSS 4, shadcn-svelte, `@lucide/svelte`), managed with pnpm.
- New projection data for the canvas: the playback target and the cast chooser list, artwork, chapters and intro/credits marks, trickplay frames, the video/audio/subtitle track lists, and per-row and queue finish clocks. Design D4 and its Open Questions say where each comes from.
- New system dependency for building `mbv-gui` on Linux: WebKitGTK 4.1 and its GTK3 stack.
- CI: the `build` and feature-matrix jobs install `webkit2gtk-4.1` and pnpm, and build `ui/` before `cargo clippy --workspace` and `cargo hack check --workspace`, because `tauri::generate_context!` needs `frontendDist` at compile time. A new GUI job runs `cargo nextest run -p mbv-gui` and the front-end gates (type check, lint, unit tests). The local `cargo clippy --workspace` gate also needs WebKitGTK and a built `ui/`.
- Docs: `AGENTS.md` (repository map, tokio edge list), `CONTEXT.md` (GUI Client definition), a GUI Client architecture guide, and `docs/recipes.md` where a recipe gains a GUI site.
