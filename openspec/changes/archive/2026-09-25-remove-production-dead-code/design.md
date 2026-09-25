# Design

## Context

The current tree contains item-level, module-level, and test-module `dead_code` suppressions across the shared list/media-list seams, render theme and tree-browser modules, destination content, shell/input/queue components, and `src/mpris.rs`. The issue's inventory is a starting point rather than a reason to delete by location: a declaration is retained only when a non-test build can reach it. The repository's acceptance gate is `cargo clippy --workspace --all-targets -- -D warnings`, alongside hermetic tests and `cargo fmt`.

The cleanup spans the `mbv` binary and a small amount of `mbv-core` test support, but it has no wire-format, queue, playback, persistence, or user-visible contract. Existing test-only helper arrangements may be useful to tests of live code; they are not automatically dead merely because they are not used by production.

## Goals / Non-Goals

**Goals:**

- Establish a complete, reviewable inventory of `dead_code` suppressions and classify every affected declaration by production reachability.
- Delete declarations and their test-only consumers when no production caller exists.
- Remove suppressions from declarations that are production-reachable, leaving the compiler to enforce future use.
- Remove the speculative list-seam convention and stale session-connect rationale when their underlying items are deleted.
- Keep each removal batch independently compilable and reviewable.

**Non-Goals:**

- Changing runtime behavior, public/user-facing APIs, protocols, queue authority, playback, or persistence.
- Designing a replacement for an item that has no production consumer, including a new test-only compatibility API.
- Removing tests that still verify behavior of live production code, or refactoring unrelated test architecture.
- Adding a different lint suppression, weakening lint configuration, or changing feature/cfg behavior to make unused code compile.

## Decisions

1. **Use compiler diagnostics as the reachability oracle, with structural search as a map.** For each subsystem, remove the suppressions temporarily, run the applicable production and all-target checks, and inspect the resulting warnings together with references and `cfg`/feature gates. A public visibility, a documentation promise, or a test call is not evidence of production reachability; conversely, indirect trait, macro, platform, or feature-gated paths are checked before deletion. No custom checker or new lint tool is introduced.

2. **Classify before deleting.** Each affected item receives one of three outcomes:
   - **Live:** production code reaches it under a supported build configuration; retain the item and delete only its `dead_code` attribute.
   - **Test-only:** no production path reaches it; remove the item, then remove only tests, helpers, imports, module declarations, and comments that exist solely for that item.
   - **Unproven/conditional:** pause that item until the relevant production target or feature is checked; do not leave a speculative allow as the resolution.

   Tests that exercise a live item remain, even when the test is the only direct caller of a small helper. Tests whose subject is deleted are removed rather than rewritten around a replacement seam.

3. **Work from shared seams outward.** Remove dead members from `src/app/components/list/` and `src/app/components/media_list/` together with their render counterparts first, then clean render theme/tree-browser items, destination content (`feeds_content`, `tv_content`, `music_content`, and library-panel content), and finally shell/input/queue/component and `mpris` items. This order prevents a shared helper from being deleted while a production destination still needs it and makes each batch's compiler result interpretable. Test-only `mbv-core` support is handled with the affected item rather than as a separate feature.

4. **Delete at the declaration boundary.** A production-unreachable item is removed with its directly dependent test-only scaffolding, not left as a renamed alias or a public wrapper. Cleanup follows the compiler: remove stale imports, `use` re-exports, test module declarations, and empty helper files where appropriate. The shared-list module documentation and session-connect comments are edited only after the convention/primitive they describe is actually removed.

5. **Use small, family-sized batches.** Each batch records its affected files and verification result, and is kept within a reviewable subsystem. The final cleanup is accepted only when the direct suppression inventory is empty (apart from ordinary `unused_imports` cleanup where no `dead_code` allowance remains) and all workspace targets remain warning-free. Formatting and file-line checks are gates, not reasons to split unrelated code.

## Risks / Trade-offs

- [A false dead-code classification breaks an indirect or conditional production path] → Check all relevant workspace targets, feature/cfg combinations represented by the code, and call sites before deletion; retain and unsuppress any live item.
- [Deleting a test helper also removes coverage for a live behavior] → Trace each test's assertions to production behavior and remove only tests whose subject is the deleted declaration.
- [Deleting a shared seam item causes a large cascade of visibility/import failures] → Process shared seams first, compile after each family, and fix the cascade at the declaration boundary rather than introducing aliases or allows.
- [The broad diff hides an accidental behavior change] → Keep batches small, preserve existing tests for live paths, and review the exact deletion inventory alongside `cargo nextest` and clippy output.
- [A module-level attribute exposes unrelated warnings in test-only code] → Classify and remove or gate the affected test support at its source; do not replace the module-level allowance with another blanket suppression.

## Migration Plan

There is no runtime or data migration. Apply the cleanup in subsystem batches, run targeted checks after each batch, and land each batch as a separate reviewable commit. If a classification proves wrong, restore that batch's commit or retain the declaration without its suppression; the final workspace check is the integration gate. Rollback is therefore a normal source revert.

## Open Questions

None. The issue's desired outcome is clear; implementation may retain a specific item when its production reachability is proven, but that does not change the approach or acceptance criteria.
