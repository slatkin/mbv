# Tasks

## 1. Shared Three-Line Presentation

- [x] 1.1 Add the target-keyed three-line flat control and one Render Component over the existing `RowFlow`/`Cursored`/`Viewported`/`PaintRetained` seam, with a configurable separator gap; verify with focused state and buffer tests for selection, stripe parity after scroll, short heights, full-card hit bounds, stale-paint invalidation, and gap changes (`cargo nextest run -p mbv -E 'test(three_line)'`). Keep one-line media rows and the tree unchanged.

## 2. F3 Target Boundary

- [x] 2.1 Define a kind-qualified stable F3 target key and replace index-based `SelectSession` request/dispatch with current-snapshot identity lookup; verify a missing/reordered key cannot connect a different target, including equal Emby/Cast ids (`cargo nextest run -p mbv -E 'test(session)'`).
- [x] 2.2 Embed the three-line control in `SessionsComponent`, project existing Emby/Cast text and aqua `✚`, and delegate key/mouse selection and hit resolution; remove its numeric cursor/scroll/row hit map and old populated-card painter while retaining sidebar chrome, empty/loading, refresh/detach/dismiss, and existing wheel behavior. Update existing focused component/buffer tests for the selected-row bar, aqua badge, item-based zebra, and unfilled separators; verify `cargo nextest run -p mbv -E 'test(session)'`.

## 3. Integration and Acceptance

- [x] 3.1 Extend existing mounted `Application::tick()` tests to verify F3 focus, selection and pointer delivery through shell synchronization and identity-bearing activation after a refreshed snapshot; check narrow and ordinary widths and missing-target handling (`cargo nextest run -p mbv -E 'test(tick_integration)'`).
- [ ] 3.2 Run `cargo fmt`, `cargo check -p mbv`, `cargo nextest run -p mbv`, and `cargo clippy --workspace --all-targets -- -D warnings`; review one painter/one owner at short and normal heights and manually inspect a longer F3 target list for spacing (manual visual check, not a live automated test). Adjust only the gap setting if the user requests denser cards.
- [ ] 3.3 Update `CONTEXT.md` with the new three-line presentation term and sync the approved delta requirements into `openspec/specs/` with the implementation; verify `openspec validate three-line-flat-list --strict` and confirm the change is ready for user visual acceptance before archiving.
