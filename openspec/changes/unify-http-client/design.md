## Context

See proposal.md for the scope history — this is the trimmed version after two review passes. **PR #555 is merged** (`81383e4f` on `origin/main`); all code references below were re-verified directly against current `origin/main`, not the old branch. That merge already:
- fixed the TLS-provider bug in all 5 agent-construction sites,
- deduplicated agent construction *within* `mbv-core` (both `EmbyClient::new` and `AudiobookshelfClient::new` call `crate::native_tls_agent`),
- deduplicated agent construction *within* `src/app` (`images.rs`'s two sites both call `super::feed_parse::tls_agent`).

The one remaining gap: `mbv_core::native_tls_agent` (`crates/mbv-core/src/lib.rs`) and `feed_parse::tls_agent` (`src/app/feed_parse.rs`) build the identical `TlsConfig::builder().provider(NativeTls).build()` agent, independently, because the former is `pub(crate)` and the latter — living in a different crate — can't call it.

## Goals / Non-Goals

**Goals:**
- Zero duplicate copies of the TLS-config incantation across crates.

**Non-Goals:**
- No shared failure-classification module (`HttpFailure`, `classify`) — dropped per the second review pass. `EmbyClient::service_failure` and `audiobookshelf_catalog::map_error` stay as they are; deduplicating them doesn't fix a bug and doesn't have a crate-boundary reason to exist, unlike the agent builder.
- No shared send/decode/header layer (unchanged from the original review's Non-Goals).
- No new module, no new file. `native_tls_agent` stays where it is in `lib.rs`; only its visibility changes.

## Decisions

**Change visibility, don't relocate.** `native_tls_agent` already lives in the right place (`mbv-core`, the shared base both `src/` and `crates/mbvd/` depend on) and already has the right shape. The only defect is `pub(crate)`. Making it `pub` and pointing `feed_parse::tls_agent` at it is the entire fix — no new abstraction needed.

**`feed_parse::tls_agent` keeps its existing signature** (`fn tls_agent(global_timeout: Option<Duration>) -> ureq::Agent`), including its `pub(super)` visibility and its name — `src/app` callers (`feed_parse.rs`, `images.rs`) don't need to change at all. Only its body changes, from the inline `Agent::config_builder()...` incantation to `mbv_core::native_tls_agent(None, global_timeout)`.

## Risks / Trade-offs

- **None material.** This is a one-line visibility change plus a ~10-line body replacement with identical output. The existing test suite (unit tests already passing on the `62d789d8` state) is sufficient verification — no new tests needed for a change with no new logic.

## Migration Plan

1. `git pull` / branch from current `origin/main` (`81383e4f` or later) — #555 is already merged, this is fresh work on top of it, not a branch amendment.
2. Change `crates/mbv-core/src/lib.rs`: `pub(crate) fn native_tls_agent` → `pub fn native_tls_agent`.
3. Change `src/app/feed_parse.rs`: replace `tls_agent`'s body with `mbv_core::native_tls_agent(None, global_timeout)`.
4. `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets`, `cargo nextest run --workspace` — expect no output changes, since behavior is unchanged.
5. Open as its own small PR.

## Open Questions

None.
