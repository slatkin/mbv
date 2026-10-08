# Design

## Context

See proposal.md. Rules: `docs/standards/rules/M-MODULE-DOCS.md`, `M-DOC-INLINE.md`, `M-DOC-FIRST-SENTENCE`. Existing root docs to match in tone and length: `mbv-keybinds/src/lib.rs`, `mbv-emby-model/src/lib.rs`. `pub use` lines today: about 190 across 17 crate roots plus nested modules. There are no glob re-exports, and all re-exported items come from this workspace.

## Goals / Non-Goals

**Goals:**
- Every library crate root opens with a `//!` that says what the crate owns and what it does not.
- Every workspace-item `pub use` renders inline in rustdoc.

**Non-Goals:**
- No rustdoc publishing, no new lint, no `cargo doc` CI job. The rustdoc lint `missing_crate_level_docs` only fires under `cargo doc`, so it would need its own CI step to enforce anything.
- No doc comments on individual items. Add one only where it is needed to make a new `//!` accurate.

## Decisions

- **Real docs, not stubs.** The issue asks for a handful done properly rather than 21 stubs. Every crate gets 2-6 lines: a one-line summary, then the ownership boundary and the neighbour crate that owns what this one does not. Source the boundary from the AGENTS.md repository map and `CONTEXT.md`, then check it against the crate's `mod` list and `Cargo.toml` dependencies. Use `CONTEXT.md` terms and avoid its *Avoid* terms. Rejected: one-line stubs, which satisfy the rule but explain nothing.
- **`mbv-desktop` and `mbv-daemon` docs follow the AGENTS.md headless-versus-Local-process split.** The `mbv-daemon` root says it is the library shared by `mbvd` and the TUI in-process. The `mbv-desktop` root says it owns the tray and MPRIS, which `mbvd` never has.
- **One `#[doc(inline)]` per `pub use` statement.** A multi-line `pub use x::{a, b, c};` is one statement and takes one attribute. Do not split statements.
- **Inline only workspace items.** Re-exports of `mbv_*` crates and `self::`/`super::`/`crate::` paths are inlined. Anything from `std` or a third-party crate stays plain. The implementer checks each `pub use` path as they edit.
- **`cfg`-gated re-exports** (for example `mbv-config` `test_support`) get the attribute too; it composes with `#[cfg]`.
- **Same-crate re-exports are included.** M-DOC-INLINE says to inline items that "fit in with their siblings". A `pub use` of a private child module's item qualifies, as the issue's examples show.

## Risks / Trade-offs

- [Docs state a boundary that is wrong] → Derive each boundary from the crate's modules and dependencies, not from memory. Prefer fewer, true sentences.
- [Docs rot with no check] → Accepted. The cost is low for a one-maintainer repo, and the rule text is the standard. A lint is a separate decision.
- [Inlining changes the doc path of an item] → No effect on code paths or semver. No rustdoc is published.
- [A `//!` line breaks `cargo fmt` or clippy `doc_markdown`] → Clippy pedantic is on. Backtick identifiers in the docs. Run `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt`.
