# Complete Canonical Media-List Ownership — Retired Campaign Record

This planning-only ledger is retained as a historical record. It is not an active campaign queue and does not authorize proposal creation or implementation.

## Accepted baseline records

- [x] 1.1 Accept PR #684 as the Queue and Grouped Music painting/geometry foundation; evidence: PR `https://github.com/slatkin/mbv/pull/684`, merge `f647136a`, archived change `openspec/changes/archive/2026-09-09-repair-canonical-media-list-ownership/`, synced main specs, checked automated gates, and checked the human-verification task.
- [x] 1.2 Reconcile the historical visible-screen inventory, preserved screen-specific workspaces, two-column catalog boundary, exclusions, and fixed order; evidence: the historical `proposal.md`/`design.md` records and issue comment `https://github.com/slatkin/mbv/issues/681#issuecomment-5597346374`.
- [x] 1.3 Reconcile non-grouped Music views against issue #681; decision recorded in `577683a1`: V1 album-folder states whose levels do not start with `group`, V2 transient group-list roots, V3 non-album intermediate levels, and V4 deeper-than-configured levels were outside this campaign and the Grouped Music follow-on. Evidence included `src/app/music_actions.rs:9-29,241-289`, `src/app/lib_cursor_actions.rs:57-73`, `src/app/shell_library.rs:71-76`, `src/app/shell_music_workspace.rs:33-48,95-105`, `src/app/shell_browser.rs:185-203`, `src/app/render/components/widgets.rs:515-603`, `src/app/render/tests_music_characterization.rs:111-136`, `crates/mbv-core/src/config_parse.rs:165-177`, and `crates/mbv-core/src/config_tests_library.rs:9-19`. This records campaign scope only; it does not classify those views as correct or complete and creates no backlog commitment.

## Withdrawn future work

The former sections 2–9 and rows 2.1 through 9.5 are withdrawn, not complete. They are intentionally absent from this retired ledger because proposal generation and fixed implementation ordering do not belong inside a campaign execution plan. No replacement proposal or implementation is implied.

The accidental planning-only row-2.1 artifacts were created in `5030adcd` and removed by revert `bd243f34`. No Rust/source files, tests, or behavioral specs were changed by that attempt.

## Closure record

- This record is archived after the accepted baseline rows remain traceable.
- Issue #681 is superseded as an active campaign tracker.
- Future work must begin with an interactive user-selected exploration, explicit scope authorization, one bounded proposal, and a separate apply request.
