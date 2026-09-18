# Tasks

## 1. Spikes (block everything below)

- [ ] 1.1 Confirm `const fn color(self) -> Color` with a full match compiles under MSRV 1.88 (const-match stable since 1.46, expected to pass) and that `serde_json` is available to `mbv` dev-dependencies (it is a workspace dep, expected to pass); record both outcomes in design.md and verify by pasting the check commands' output into the task trail
- [ ] 1.2 Produce the 29-variant name table as `openspec/changes/palette-enum/name-table.md` (columns: old primitive(s) → new PascalCase variant → hex → one-word description) following the naming rules in design.md: keep existing color-words (`Gold`, `Aqua`, `Red`, `Orange`, `Purple`, `Foam`); number grey cluster (`Grey1`–`Grey6`) and dark green-grey cluster (`Green1`–`Green3`); blue-grey slates are `Slate`/`Storm`/`Flint`; plain hue names elsewhere; ordered by hue then dark-to-light. Get user review; verified by an approved table committed in the change

## 2. Palette core (additive, no callers)

- [ ] 2.1 Add `render/theme/palette.rs` with the `Palette` enum, `const fn color()`, `ALL`, and hex/name accessors; verify `cargo check -p mbv` passes
- [ ] 2.2 Add palette-value uniqueness test over `ALL` plus the `docs/palette.json` sync test; verify with `cargo nextest run -p mbv` for the new tests

## 3. Type migration

- [ ] 3.1 Migrate the 51 role consts (47 production, 4 test-only) to `Palette` assignments and delete duplicate-literal primitives and primitive→primitive aliases; verify `cargo check -p mbv` produces only type-mismatch errors at `palette::` call sites (expect ~65 files with `expected Palette, found Color`), no errors in theme internals
- [ ] 3.2 Migrate surface rows, level fills, and `surface_colors` to return `Palette`, converting to `Color` at the paint boundary; verify `cargo check -p mbv` is clean except call sites
- [ ] 3.3 Perform the mechanical call-site migration (`.fg(role)` → `.fg(role.color())`) across the ~65 `palette::` consumers; migrate `HERO_META_ROLES` and `HINT_PILL_FILLS` from `[Color; 3]` to `[Palette; 3]` and update their call sites to convert at use; verify `cargo check -p mbv` is fully clean

## 4. Cleanup and proof

- [ ] 4.1 Bridge frozen-test names as thin test-only `Color` shims, delete `primitives.rs` and the "deliberately not X" comments, document the raw-`Color` residual rationale; migrate stray production `Rgb` literals in `chrome_tabs.rs:168`, `media_list.rs:413,478` to palette role references (test-only literals in `library_hero_overlay.rs` are known residual); verify frozen test files have no resolved-value changes (`cargo nextest run` for the frozen tests green, `git diff --stat` shows no frozen files touched)
- [ ] 4.2 Regenerate `docs/palette.json` from the enum: variant-first structure (each variant carries name, hex, and list of roles assigned to it); restructure `docs/palette.html` to group by palette variant with roles listed under each (replaces the current role-first layout); verify the sync test passes and every resolved value matches main (full `cargo nextest run -p mbv`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`)
