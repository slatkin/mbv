# Proposal

## Why

Issue #890: 21 of 25 library crate roots have no `//!` documentation, and no `pub use` re-export carries `#[doc(inline)]`. Crate roots such as `mbv-ctrl` (`mod commands;`) and `mbv-queue` (`use std::collections::HashMap;`) say nothing about what the crate owns. Re-exports render as an opaque list. This breaks `M-MODULE-DOCS` and `M-DOC-INLINE` (`docs/standards/rules/`), which apply to internal crates too. The readers who pay are people and agents working in the repo, since no rustdoc is published.

## What Changes

- Add a `//!` block to each of the 21 crate roots that lack one. Each states what the crate owns and what it does not, drawn from the AGENTS.md repository map, `CONTEXT.md` terms, and the crate's actual code. The first sentence is a single line (`M-DOC-FIRST-SENTENCE`).
- Add `#[doc(inline)]` to every `pub use` in a library crate that re-exports a crate-local or sibling-workspace-crate item.
- No `std` or third-party re-export is inlined (none are expected today).
- No behavior, public API, dependency, or lint-config change. **No BREAKING changes.**

## Capabilities

### New Capabilities
None.

### Modified Capabilities
None. This is documentation only, so the change sets `skip_specs: true`.

## Impact

- `crates/*/src/lib.rs` for the 21 crates named in the issue: mbv-audiobookshelf, mbv-cast, mbv-config, mbv-core, mbv-ctrl, mbv-daemon, mbv-desktop, mbv-emby, mbv-feed, mbv-ids, mbv-images, mbv-net, mbv-player, mbv-queue, mbv-remote-player, mbv-render, mbv-text, mbv-theme, mbv-ui-model, mbv-visualizer, mbv-ws.
- Nested `pub use` sites in library crates, for example `mbv-render/src/components/*`, `mbv-components/src/{list,library_panel,media_list}.rs`, `mbv-queue/src/items.rs`, `mbv-emby/src/types.rs`, `mbv-core/src/applog.rs`, `mbv-remote-player/src/connect.rs`, `mbv-ui-model/src/audiobookshelf_browse.rs`.
- Out of scope: the `mbv` and `mbvd` binary crates, the four crates that already have root docs (their `pub use` lines are still inlined), publishing rustdoc, and a lint to enforce either rule.
