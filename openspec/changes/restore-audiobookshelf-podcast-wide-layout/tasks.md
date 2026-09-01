## 1. Pin current geometry

- [ ] 1.1 Confirm clean accepted base `35df6f6`, PR #606 feature-branch stacking, and issue #640 ownership; record that canonical-list work and other feed-view changes are out of scope.
- [ ] 1.2 Add/retain metadata-bearing Audiobookshelf podcast TestBackend characterization at widths 81, 82, and a larger Wide size. Assert the current threshold transition, right/hero rects, one-vs-provider column behavior, and selected-row scroll/re-anchor behavior before replacing it.
- [ ] 1.3 Characterize the shared minimum-height fallback at Wide width and preserve the Narrow capture, including images enabled and disabled.

## 2. Restore shared Wide placement

- [ ] 2.1 Make the shell/component Wide path consume the shared Hero-on-left geometry and shared pill-bar/right-pane helpers; remove the podcast-specific detached-detail placement without adding a new breakpoint.
- [ ] 2.2 Render the Wide right rail as one fixed-row column with shared semantic surface/backdrop/focus framing and Wide pills. Leave non-podcast column policies untouched.
- [ ] 2.3 Keep the existing Audiobookshelf episode workspace, episode targets, provider state, artwork, and typed intents intact; update geometry projections/overlay anchors only where the shared rects require it.
- [ ] 2.4 Preserve Narrow inline hero and its re-anchor/scroll, selector, explicit-child, and image-disabled behavior. Split `src/app/render/components/audiobookshelf_podcast.rs` into a cohesive module only if the 800-line cap would otherwise be exceeded.

## 3. Verification and delivery

- [ ] 3.1 Add focused buffer/geometry assertions for Wide 82 and larger widths: hero-left placement, pill row, one-column row x positions, rail framing, marker alignment, and episode workspace. Include short Wide fallback and Narrow regression cases.
- [ ] 3.2 Run `rtk cargo fmt --all -- --check`, `rtk cargo check --workspace --all-targets`, relevant Audiobookshelf podcast tests via `rtk cargo nextest`, and `rtk make check-code-file-lines`.
- [ ] 3.3 Run `rtk openspec validate restore-audiobookshelf-podcast-wide-layout --strict`; resolve implementation-caused failures before delivery.
- [ ] 3.4 Manually inspect the real Audiobookshelf podcast surface at 81, 82, and larger Wide dimensions plus a short terminal: hero-left, pills, episode workspace, images, selection, and Narrow fallback. Record evidence in the implementation review.
- [ ] 3.5 Commit as one independent implementation slice on the PR #606 feature branch; do not mark umbrella tasks or fold canonical-list work.
