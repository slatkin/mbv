## Context

The rename plan (2026-07-24) executed two commits: deletion of Standard view (commit `860e672`) and a mechanical rename of the queue-side/library-side naming collision (commit `b51cb82`). The rename deliberately scoped itself to the collision described in ADR 0013 — it did not touch every `power_`-prefixed identifier. Issue #402 captures the remaining references: doc comments, module docs, user-facing status strings, test function names, and plan/ADR documents that still use "Power View" terminology.

This is a text-only change. No logic, data flow, or rendering behavior changes. Every edit is a rename in comments, documentation strings, string literals shown to users, or test function names.

## Goals / Non-Goals

**Goals:**
- Eliminate every remaining "Power View" reference from source code comments, doc comments, user-facing strings, and test names.
- Update docs (ADR 0013, ADR 0009 amendment, rename plan, other plans) to reflect current terminology.
- Keep `cargo build`, `cargo clippy`, and `cargo test` green throughout.

**Non-Goals:**
- Renaming identifiers that use `power_` as a prefix but do not reference "Power View" (e.g. `render_power_home_list`, powerline styling, `power_home_actions.rs` filenames). These are implementation details outside ADR 0013's scope.
- Changing any runtime behavior, rendering, or input handling.
- Restructuring modules or moving files.

## Decisions

### 1. Split into two PRs: source code and docs

**Choice:** Source code changes and doc changes go in separate PRs.

**Rationale:** The issue guidance recommends this. Source code changes touch ~25 files with mechanical renames; docs changes touch 5 files with prose edits. Separate PRs keep each review focused and reduce the chance of a rename mistake hiding in a large diff.

### 2. Use Serena `rename_symbol` for test function renames

**Choice:** Use Serena's reference-aware rename for test function names rather than grep-and-replace.

**Rationale:** Test renames are mechanical but scattered across many files. `rename_symbol` ensures call sites (e.g. `#[test]` invocations, module paths) stay consistent. For doc comment and string literal changes, direct edits are appropriate since they are not symbol renames.

### 3. User-facing string changes are behavioral (test assertions must update)

**Choice:** When changing status bar strings like `"Power view width: ... cols"`, update the corresponding test assertions in the same commit.

**Rationale:** Tests assert on the exact string content. Changing the string without updating the assertion breaks the test. These must stay in sync.

### 4. ADR 0013: amend rather than rewrite

**Choice:** Add a clarifying amendment to ADR 0013 noting that remaining `power_` references are implementation details, not view terminology. Consider renaming the ADR title if it still says "Power View Is The Only View" after this cleanup.

**Rationale:** The ADR's historical content is valuable. Rewriting it loses the decision record. An amendment preserves the original reasoning while noting the cleanup.

## Risks / Trade-offs

- **[Over-renaming]** → Mitigation: Stick strictly to references that say "Power View" (any case/separator form). Do not rename `power_`-prefixed identifiers that are module/file names or implementation details. When in doubt, leave it.
- **[Breaking test assertions]** → Mitigation: Run `cargo test` after every file change. Update assertions in the same commit as the string change.
- **[Large diff hard to review]** → Mitigation: Split into source-code and docs PRs. Within source code, group by file type (comments first, then strings, then test names).
