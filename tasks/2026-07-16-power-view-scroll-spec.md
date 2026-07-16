# Spec: Power View Music Scroll Feel and Album-Block Visibility

## Objective

Preserve and verify natural scrolling behavior in Power View music album browsing, especially when inline album detail is visible. The concrete user problem is that scrolling down can leave the selected album block pressed against the bottom of the viewport so the user mostly sees titles and not the whole block. Issue `#189` is the narrow album-block case; issue `#205` is the broader UX framing.

This spec is being written on **Thursday, July 16, 2026** because behavior appears better after code pushed today, but it is still unclear whether that improvement is:

- a durable fix,
- an indirect side effect of unrelated Power View work,
- or a terminal-size-dependent coincidence.

Success means mbv has an explicit, testable expectation for this scrolling behavior so it does not silently regress later.

## Tech Stack

- Rust 2021
- `ratatui` for terminal rendering
- `crossterm` for input
- Power View library rendering/input code under `src/app/render/power/` and `src/app/input.rs`

## Commands

- Build: `cargo build`
- Test: `cargo test`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format check: `cargo fmt --all -- --check`

## Project Structure

- `src/app/render/power/album.rs` → grouped music album display plan and grouped-row rendering
- `src/app/render/power/music.rs` → Power View music-group rendering entry point
- `src/app/render/power/list.rs` → shared Power View left-panel list rendering
- `src/app/input.rs` → keyboard paging and navigation behavior
- `tasks/` → planning/spec artifacts

## Code Style

Match existing Power View behavior-focused tests: describe the rendered-row situation precisely, then assert the cursor/scroll result.

```rust
assert_eq!(
    app.libs[0].nav_stack.last().unwrap().cursor,
    expected_cursor,
    "paging should follow rendered display rows, not raw album count"
);
```

Conventions:

- Reuse the existing grouped-album display model instead of inventing a parallel scroll model.
- Keep behavior decisions local to Power View music album browsing.
- Add the minimum state/assertions needed to express the UX rule.

## Testing Strategy

- Primary: focused unit/render tests around grouped music album scrolling behavior.
- Secondary: full regression run with `cargo test`.
- Manual check remains necessary because this is a UX/viewport issue and unit tests are not a source of truth for feel.

Tests should cover:

- a viewport tall enough to fit the full selected album block,
- a shorter viewport where the full block cannot fit,
- downward movement near the bottom of the viewport,
- behavior across different inline states: loaded tracks, loading placeholder, no inline detail,
- page navigation and single-step navigation separately.

## Boundaries

- Always: preserve the current code as the source of truth for actual behavior, state exact dates when discussing recent changes, and write acceptance criteria in viewport terms rather than vague “feels better” language.
- Ask first: changing the grouped-album display model in a way that also changes artist-header behavior from `#219`, or broadening the fix beyond Power View music album browsing.
- Never: claim `#189` or `#205` is fixed solely because current behavior happens to look good on one terminal size.

## Success Criteria

- There is a stable spec for Power View music scrolling behavior even if the current implementation happens to look acceptable today.
- When the viewport is tall enough to fit the selected album block, scrolling down keeps that block fully visible whenever practical instead of pinning it to the bottom edge unnecessarily.
- When the viewport is too short to fit the full block, scrolling chooses the least surprising partial view rather than a bottom-anchored view that hides most of the block.
- Acceptance tests can distinguish a genuine block-visibility policy from an accidental terminal-size-specific good result.
- The spec explicitly records relevant recent code history:
  - `dece032` on **July 13, 2026** changed Power View album paging to follow rendered display rows.
  - `e113100` on **July 16, 2026** changed grouped music selection behavior and touched the same files, but was not authored as a scroll fix.

## Open Questions

- Is the currently improved behavior coming from the July 16, 2026 `#219` push, from the earlier July 13, 2026 paging fix, or from the interaction between them?
- Does the problematic bottom-anchored feel still reproduce at shorter terminal heights or with different inline detail heights?
- Should `#205` be treated as fully subsumed by `#189`, or should it remain a broader “viewport policy” issue even if the album-block case is fixed?
