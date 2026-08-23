## 0. Prerequisite

- [ ] 0.1 `unify-selected-row-background` is landed; hero-on-left selection rows
  call the shared primitive, not per-surface paints.

## 1. Podcasts wide pill bar

- [ ] 1.1 In `render_audiobookshelf_podcasts` wide branch, replace the direct
  `render_audiobookshelf_show_rows(f, right_panel, …)` with the book-browser
  shape: `hero_on_left_right_pane` → `render_audiobookshelf_podcast_bucket_pills`
  into `pills_area`, show list into `list_panel`.
- [ ] 1.2 Delete the "Wide mode has no equivalent row" comment (audiobookshelf.rs
  ~160-164).
- [ ] 1.3 → verify: wide podcast renders the bucket pill bar; selected bucket
  matches narrow for the same selected show (buffer check).

## 2. Emby podcast de-specialization

- [ ] 2.1 Remove `|| self.is_podcast_library(lib_idx)` from `list.rs:136`; remove
  the podcast branches at `list.rs:241`/`:263` and `detail.rs:94`/`:343`.
- [ ] 2.2 If `is_podcast_library` (`feed_actions.rs:386`) is now unused, delete
  it; if Feeds still uses it, keep the fn, drop only the render callers.
- [ ] 2.3 → verify: an Emby podcast-collection library renders with the same
  arrangement a generic library of its shape uses (buffer check against a real
  podcast library).

## 3. Verify

- [ ] 3.1 `rtk cargo nextest run -p mbv`, `rtk cargo clippy --workspace
  --all-targets`, `rtk make check-code-file-lines`.
- [ ] 3.2 Manual: wide podcasts show pills; Emby podcast library looks generic;
  no per-surface selection paint reintroduced.
