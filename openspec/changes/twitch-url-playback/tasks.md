# Tasks

## 1. Twitch item type (`mbv-queue`)

- [x] 1.1 Add `crates/mbv-queue/src/twitch.rs` with `TwitchItem { url, title }`, `TwitchUrlError` (domain error type), and `TwitchItem::from_url` per design D2. Re-export from `lib.rs`. Verify with a `#[case]` table in `crates/mbv-queue/tests/queue/` (contract: host check of spec *The prompt accepts only Twitch hosts*): live, VOD, clip pass; other host, look-alike host, and non-http scheme fail; plus one case per distinct derived title. `cargo nextest run -p mbv-queue`.
- [x] 1.2 Add `QueueItem::Twitch`, `QueueItemKind::Twitch`, `QueueItemContentId::Twitch`, `MpvUrlSource::Twitch`, and the `"Twitch"` serde tag in `QueueItemSerialize` and the manual decode (design D1). Give each `QueueItem` method in `items.rs` an explicit arm. Extend the existing serde round-trip test with one Twitch case (contract: spec *A Twitch item persists with the queue*). `cargo nextest run -p mbv-queue`.
- [x] 1.3 Extend the existing owner-admission test with one case: an audio-only owner refuses a Twitch item (spec *Only mpv output plays a Twitch item*, audio-only scenario). `cargo nextest run -p mbv-queue`.
- [x] 1.4 Add *Twitch item* to `CONTEXT.md` (Queue section): what it holds, that only mpv plays it, that it keeps no playback state. Verify the term appears and names its *Avoid* terms.

## 2. Workspace match arms

- [x] 2.1 Give every `match` on `QueueItem` outside `mbv-queue` that fails to compile an explicit Twitch arm per design D4 and D8 (no wildcard). Covers `crates/mbv-player`, `crates/mbv-daemon` (except Cast and ctrl, groups 3 and 4), `crates/mbv-desktop`, `crates/mbv-components`, `crates/mbv-render`, and `src/`. Add one line to `docs/invariants/06-queue-progress-application-sites.md`: a Twitch item applies no progress. Verify `cargo check --workspace --all-targets`.
- [x] 2.2 Implement design D3: `mpv_url_for_queue_item` returns the Twitch URL unchanged; `prepare_source` sends it through `PreparedSource::plain`. Add one unit test in `mbv-player` (contract: spec *mpv receives the Twitch URL unchanged*). `cargo nextest run -p mbv-player`.

## 3. Outputs

- [x] 3.1 `prepare_cast_source` returns `Uncastable` for a Twitch item through a new `CastError` constructor that names the item (design D5). Add one case to the existing Cast resolve tests (contract: Cast scenario of spec *Only mpv output plays a Twitch item*). `cargo nextest run -p mbv-daemon -p mbv-cast`.
- [x] 3.2 Add one case to the existing Emby Session `admit` tests: a Twitch item is refused and named (no code change expected). `cargo nextest run -p mbv-daemon`.

## 4. Ctrl capability `twitch-queue`

Follow `docs/recipes.md`, *Add a ctrl protocol variant*, steps 1 to 8.

- [x] 4.1 Add `CTRL_CAP_TWITCH_QUEUE`, advertise it in `CtrlHello::current()`, add `supports_twitch_queue`, and set it in `DaemonHelloCaps`. Add a handshake case in `crates/mbv-ctrl/tests/ctrl/handshake.rs` and the queue-shape round trip in `queue.rs`. `cargo nextest run -p mbv-ctrl`.
- [x] 4.2 Record the capability in `HelloClassification` and on the client record. Filter Twitch slots for incapable peers in `unified_queue_state_for_peer` and `broadcast_state_gated`. Add `twitch_queue_transport_rejection` beside `abs_queue_transport_rejection` and call it at the same sites. Extend the existing filter and rejection tests with one Twitch case each (contract: spec delta *Twitch item transport is additively negotiated*). `cargo nextest run -p mbv-daemon`.
- [x] 4.3 Check the `CtrlEvent` decode in `crates/mbvd/src/main.rs` still matches. `cargo nextest run -p mbvd`.
- [x] 4.4 Add the Twitch fixture in `protocol/ctrl/v10/` and regenerate schemas with `MBV_PROTOCOL_BLESS=1 cargo nextest run -p mbv-ctrl conformance`. Review the diff, then `cargo nextest run -p mbv-ctrl conformance`.

## 5. Prompt and keybind action

Read `docs/architecture/tui-frontend.md` first. Follow `docs/recipes.md`, *Add a keybind action*.

- [ ] 5.1 Add `TwitchUrlIntent { Submit(String), Cancel }` and `ShellRequest::TwitchUrlIntent` in `crates/mbv-ui-msg/`. `cargo check -p mbv-ui-msg`.
- [ ] 5.2 Add the painter in `crates/mbv-render/` beside `render_save_playlist_content`: input line, a refusal line when set, submit and cancel pills. `cargo check -p mbv-render`.
- [ ] 5.3 Add `TwitchUrlComponent` in `crates/mbv-components/` on the model of `SavePlaylistComponent` (design D7): owns input, refusal text, hit regions, gestures; Enter sends `Submit`, Escape or the cancel pill sends `Cancel`. One component test for each of Submit and Cancel (contract: the component emits the intent; it performs no host check). `cargo nextest run -p mbv-components`.
- [ ] 5.4 Add keybind action `twitch_url_open` (Global, default `Shift+T`, rebindable) per recipe steps 1, 2, and 5. `cargo nextest run -p mbv-keybinds -p mbv`.
- [ ] 5.5 Shell: mount the prompt on the action through the focus stack; on `Cancel` unmount; on `Submit` check the owner's Twitch capability (design D6), call `TwitchItem::from_url`, push the refusal on error, or unmount and call `submit_queue_item(QueueItem::Twitch(item), true)` on success. Add shell tests in `src/app/tests/` for: refused URL keeps the prompt mounted and leaves the queue unchanged; accepted URL submits a play-now Twitch item; owner without the capability sends nothing. `cargo nextest run -p mbv`.

## 6. Gates

- [ ] 6.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. All pass.
- [ ] 6.2 Run `make check-code-file-lines` just before push. Split any file over 800 lines along a responsibility seam.

## Workflow follow-up

- The user runs the live check: open the prompt, play a live Twitch channel, pause and resume through mbv keys and MPRIS, confirm Cast refuses it.
- Sync and archive only when the user approves the PR for merge.
