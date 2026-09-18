## 1. Shared title-reveal policy

- [x] 1.1 Add the closed `MediaListTitleReveal` policy (`Always` default, `RevealOnSelection`) to the shared list owner, with a carrier setter and a read accessor the embedded presentation exposes; verify with a focused unit test that a fresh list reports `Always` and that the policy survives a content refresh (`cargo nextest run -p mbv media_list`)
- [x] 1.2 Pass the policy from the embedded presentation into the row painter at the one `media_list_row` call seam, keeping the existing paint signature honest; verify the presenter's existing buffer tests pass unchanged, proving lists that do not declare the policy paint identical output

## 2. Reveal and marquee in the row painter

- [x] 2.1 Apply the reveal in the row painter: a row outside the list's selection paints only its primary context text (no secondary fragment, no ellipsis for it) while a selected row paints the full pair; verify with a focused buffer test covering an unselected row and the selected row on the same list
- [x] 2.2 Force the marquee path for the selected focused row of a reveal-on-selection list even when the title fits, leaving an always-revealed list's fitting title static; verify with a buffer test that injects the list's marquee clock (`set_marquee_started_at`) rather than sleeping
- [x] 2.3 Scroll a forced fitting title fully out of the window and back in `marquee_spans` instead of returning the static parts; verify with a unit test over the window at clock points through the full cycle
- [x] 2.4 Keep the unfocused selected row static: verify with a buffer test that a reveal-on-selection list without focus paints the full title truncated, not truncated to the context alone

## 3. Marquee clock key

- [x] 3.1 Key the list's marquee clock on the full marqueed text (context plus item title) instead of the primary text alone; verify with a presenter test that moving between two rows sharing a context text restarts the cycle from the held start

## 4. Podcast episode list opts in

- [x] 4.1 Declare `RevealOnSelection` on the podcast episode list, changing nothing else about its rows; verify with a shell tick integration test that an unselected episode row projects the parent podcast name with no title and the selected row projects the full title
- [x] 4.2 Verify both presentations of the podcast tab: a Wide and a non-Wide frame each show the context-only resting rows and the revealed selected row, and the Wide hero still names the selected episode's title

## 5. Docs and close-out

- [x] 5.1 Add the title-reveal term to `CONTEXT.md` beside the media-list vocabulary, with its `_Avoid_` list, so the policy has one name in specs, code, and conversation
- [x] 5.2 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv -p mbv-core -p mbvd`, then archive the change so its deltas land in `openspec/specs/` and `openspec validate --specs --strict` still passes
