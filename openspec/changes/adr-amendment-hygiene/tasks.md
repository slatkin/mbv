# Tasks

Each group is one commit (mirrors design D5's commit plan). Gate for every group: `openspec validate adr-amendment-hygiene --strict` green and the group's diffs read in full. No code, tests, or specs change in this change; "verify" below means the stated command or read-through, never acceptance testing.

## 1. Amendment rule

- [x] 1.1 Create `docs/adr/README.md` carrying the rule from design D1 (ADR = decision + rationale; amendment = decision change only; no `main`-status, no PR/milestone/issue status, no line-number anchors, dated `archive/` prefixes for change refs, point at owning spec or code symbol for current truth). Verify: file exists, rule covers all five rot classes from the audit inventory, and `AGENTS.md` ADR bullet gains a one-line pointer to it.
- [x] 1.2 Read every ADR touched later in this change against the new rule and list any additional violating sentence the scouts' inventory missed; add findings to this change's worklist (append to the relevant group below) or note "none". Verify: the list exists in the task notes and every listed sentence has a fix in a group below.
  - Notes: additional sentences found — ADR 0024:29 `shell.rs:349`; ADR 0024 Residual A table `mouse_gestures.rs:27`, `mouse_gestures.rs:13`, `input.rs:97`, `render/arrangements/chrome.rs:113` line anchors; ADR 0017 milestone #515 (also 0019). Added to 4.2 (0024) and covered by 3.3/3.5.

## 2. Verify-first resolutions (write nothing yet)

- [x] 2.1 Verify whether mbvd's filesystem/trusted-LAN ctrl auth (#523) landed in merged PR #529: read `crates/mbvd/src/` ctrl-auth setup against `openspec/specs/` and record the answer. Verify: recorded answer with path evidence; decides `CONTEXT.md` control-credential § text in 3.6.
  - Answer: landed. `openspec/specs/ctrl-protocol/spec.md` (ctrl authentication requirement) makes packaged `mbvd` use Unix-socket filesystem permissions and trusted-LAN reachability, with no application-level ctrl auth; `crates/mbvd/src/main.rs` constructs `EmbyClient` only on the `--connect emby` path and offers `--connect abs`.
- [x] 2.2 Verify Residual C (page-rows-off-global-geometry, ADR 0024/0022) status: search commits and code for its fix; record fixed-or-open. Verify: recorded answer with commit or code evidence; decides the Residual C sentence in group 5.
  - Answer: open. `App::lib_page_size` (`src/app/dispatch/actions.rs`) still computes page rows from global `layout.left_area`. Residual C stays listed as remaining; only its dead anchor is repointed to the symbol.
- [x] 2.3 Verify `CONTEXT.md` UI-state reset section's "without change to scope" wording: determine whether any Queue-scope concept survives (ADR 0034, `CONTEXT.md` Queue scope: Retired) or the phrase is generic. Verify: recorded decision; decides whether 3.6 edits that sentence.
  - Answer: leftover. The phrase refers to the retired *Queue scope* term (`CONTEXT.md` Queue scope: Retired). 3.6 rewrites it to "without change to slots or playback".
- [x] 2.4 Verify whether `openspec/changes/adopt-wide-hero/` ever existed: `git log --all --diff-filter=A` for that path and the archive listing. Verify: recorded answer; decides whether ADR 0026:46 gets an archive-prefixed ref or a deletion.
  - Answer: existed; archived as `openspec/changes/archive/2026-09-06-adopt-wide-hero/`. ADR 0026 gets the dated prefix.
- [x] 2.5 Adjudicate `Resolution::Refuse` semantics after ADR 0030: read ADR 0030's replacement text and `src/single_instance.rs:48-50` doc comment; decide which side is behind. Verify: recorded adjudication; decides the 0006 fix in 4.1. (If the code doc is behind, that one-line doc comment is this change's only code-adjacent edit.)
  - Answer: the code doc is behind. ADR 0006 already carries the 2026-09-30 amendment (refusal no longer means a Bare TUI, see ADR 0030); `refuse_local_owner` (`src/main.rs`) reports an owner process that is not accepting connections. 4.1 fixes the `Resolution::Refuse` doc comment.

## 3. 2026-08-14 status cluster (one coordinated edit)

- [x] 3.1 ADR 0014: delete the v7/open-PR-#529 status sentences; keep the multi-connection decision; point current version/capability truth at `crates/mbv-ctrl` symbols and the `ctrl-protocol` spec. Verify: diff shows no remaining "on main"/PR-status/version-number prose; decision paragraphs unchanged.
- [x] 3.2 ADR 0015: delete the amendment's dead path/line anchors and merged-PR status; keep the stay-alive decision and its self-flagged redirect. Verify: diff read; no `crates/mbv-core/src/ctrl.rs` or `local_daemon.rs:NNN` anchors remain.
- [x] 3.3 ADR 0017: replace the dead `control_queue.rs:254` anchor with `crates/mbv-daemon/src/control_queue.rs` (symbol-level); delete the "proposed, not implemented" fall-through status (shipped, then superseded — reference the superseding ADR/record instead); also delete the milestone #515 / PRs #520-522 attribution. Verify: diff read; no openspec/changes/ undated refs remain.
- [x] 3.4 ADR 0018: delete the "open PR #529 / not landed / main.rs:117-120" note; keep the service-independent decision; state mbvd's current shape only by pointing at the owning spec. Verify: diff read; no PR-status prose remains.
- [x] 3.5 ADR 0019: replace the "still does NOT admit Audiobookshelf / milestone #524 / transport planned" amendment text with the decision-level statement plus pointers (Owner admission spec, `daemon_admits`, ctrl capabilities); delete the mbv-core anchor and the #529 sentence; also delete the milestone #515 / PRs #520-522 attribution. Verify: diff read; the ADR no longer denies shipped behavior.
- [x] 3.6 `CONTEXT.md`: rewrite Owner admission § (drop milestone #524 pointer), mbvd § (drop open-PR/Emby-gated claims; fold in 2.1's answer), control-credential § (fold in 2.1), and — only if the user separately approves the 0027 sync — nothing: hero sections stay untouched in this change. Verify: `rg -n 'milestone|PR #529|Emby-gated|tracked in' CONTEXT.md` returns no stale hits; domain vocabulary untouched elsewhere.
- [x] 3.7 Cross-check the six files against each other: no file in the cluster now contradicts another or the specs. Verify: full read-through of all six diffs in one pass.

## 4. Dead-anchor repair (mechanical)

- [ ] 4.1 Fix ADR 0006 per 2.5's adjudication and repoint its `crates/mbv-core/src/ctrl.rs:43` and `local_daemon.rs:125` anchors (delete line numbers, keep symbol names). Verify: `rg -n 'mbv-core/src/ctrl|local_daemon.rs:[0-9]' docs/adr/0006*` empty.
- [ ] 4.2 ADR 0001 ("in mbv-core" → `crates/mbv-queue`), 0004 (`src/tray.rs` → `crates/mbv-desktop/src/tray.rs`), 0010 (`src/app.rs` → dispatch/session paths, `mbv-core/src/remote_player.rs` → `crates/mbv-remote-player`), 0022's dead `src/app/*.rs` set (Residual C anchor → `App::lib_page_size`), 0002 (`src/app/input.rs`, `input_resolver.rs`, `input_browse_dispatch.rs`), and 0024's dead `src/app/*` paths plus its `shell.rs:349` and Residual A table line anchors (found in 1.2). Verify: each repointed path exists in the tree (`fd` per path); line numbers deleted, not moved.
- [ ] 4.3 Prefix the undated change refs with their dated `archive/` directories: 0017, 0019 (if any survive 3.3/3.5), 0026 (per 2.4), 0028 (both refs), 0033, 0034. A ref with no archive directory (e.g. `audio-only-mixed-queue-admission`, never archived) is deleted, or it is replaced by the spec that absorbed it. Verify: every `openspec/changes/` hit in `docs/adr/` resolves to an existing directory (`rg -n 'openspec/changes/' docs/adr/` + check each).

## 5. Futures-that-arrived

- [ ] 5.1 ADR 0002: replace the "Roadmap (not scheduled)" keybinds block with a landed statement pointing at the `configurable-keybinds` spec. Verify: diff read; the block no longer describes shipped work as future.
- [ ] 5.2 ADRs 0022 and 0024: correct the Residual B (#643) sentences per the audit (closed 2026-09-05; predicates deleted) and the Residual C sentences per 2.2; correct 0024's drag/hover note to drag-shipped/hover-pending. Verify: diff read; no ADR lists a closed residual as remaining.

## 6. Final sweep

- [ ] 6.1 Run the rot-pattern sweep across all of `docs/adr/` and `CONTEXT.md` (`rg` for the inventory patterns: milestone, PR #, "on main", undated openspec/changes, .rs line anchors), check that every repo path in a touched ADR or `CONTEXT.md` section exists (`fd` per path), and confirm every remaining hit is an intentional historical/decision reference. Verify: the hit list with a one-word justification per survivor.
- [ ] 6.2 Full read-through of every file touched by this change, checked against the new `docs/adr/README.md` rule. Verify: no file violates the rule; `openspec validate adr-amendment-hygiene --strict` green.

## Workflow follow-up

- Open the PR and leave the change open through review and live checks (per `AGENTS.md`: sync/archive only after the user approves the merge).
- Archive the change after merge approval; no spec sync is needed (`skip_specs: true`).
- If approved separately, create the `sync-hero-left-specs` change for the ADR 0027 / hero-layout spec correction and the libmpv2 manifest cleanup change afterwards.
