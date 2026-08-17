Tracked in [#556](https://github.com/slatkin/mbv/issues/556).

## Why

During the ureq 2→3 migration (PR #555, **merged** as `81383e4f`), two of four independent agent constructors originally omitted TLS configuration entirely — this worked by accident on ureq 2.x (native-tls was implicitly picked up) and would have been a real bug on 3.x, where the TLS provider must be explicit per agent. A follow-up commit that shipped as part of that merge (`62d789d8`) fixed the bug and deduplicated agent construction *within* `mbv-core` (`EmbyClient`, `AudiobookshelfClient`) and *within* `src/app` (`feed_parse.rs`, `images.rs` — both now route through `feed_parse::tls_agent()`). One duplication remains, confirmed on current `origin/main`: `mbv_core::native_tls_agent()` and `src/app`'s `feed_parse::tls_agent()` contain the identical TLS-config incantation, once per crate, because `native_tls_agent` is `pub(crate)` and unreachable from `src/app`.

## Scope (revised)

An earlier draft of this proposal scoped a much larger unification — a new `web` module owning agent construction *and* shared failure classification (`HttpFailure`), used to replace `EmbyClient::service_failure` and `audiobookshelf_catalog::map_error`. A high-level plan review (Opus) flagged that the classification half doesn't pay for itself: it costs an enum, a classify function, a contextual constructor, and new tests, in exchange for deleting two ~6-line match blocks that don't share a bug or a visibility problem — both of `HttpFailure`'s would-be consumers already live inside `mbv-core`, so the `pub`/`pub(crate)` reachability argument (the actual justification for touching this at all) only applies to the agent builder, not to classification. Confirmed with the user: **trim to the agent-builder fix only.** No `HttpFailure`, no classification unification.

## What Changes

- Change `mbv_core::native_tls_agent` from `pub(crate)` to `pub` in `crates/mbv-core/src/lib.rs`.
- Replace `feed_parse::tls_agent()`'s body (the duplicate TLS-config incantation) with a call to `mbv_core::native_tls_agent(None, global_timeout)` — preserves current behavior exactly, since `feed_parse`'s callers never set a connect timeout today.
- **PR #555 is already merged** — this lands as a small, new, standalone PR against `main`, not an amendment to #555. (An earlier draft of this proposal said "fold into #555"; that was written after #555 had already merged and wasn't re-checked. Corrected here.)

## Capabilities

Internal refactor, no user-facing requirement change. `skip_specs: true`.

## Impact

- `crates/mbv-core/src/lib.rs` (one-word visibility change), `src/app/feed_parse.rs` (body swap, ~10 lines removed).
- No other files. No new dependencies, no protocol/wire-format changes, no behavior change (the TLS bug this closes was already fixed by `62d789d8`; this only removes the remaining code duplication).
