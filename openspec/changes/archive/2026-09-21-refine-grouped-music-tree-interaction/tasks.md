# Tasks

## 1. Artist-root Enter opens its Hero

- [x] 1.1 Make the Library panel's `hero_overlay_enter_available` report true for a Grouped Music artist root only while the tree filter is inactive, so non-Wide Enter reaches the overlay path without bypassing filtered interaction; verify with panel/owner tests for unfiltered artist eligibility, filtered artist ineligibility, and unchanged album-leaf behavior
- [x] 1.2 Replace the unfiltered `MusicContent` Enter-on-artist expansion arm with the Right chord's Hero entry (Wide takes `enter_artist_workspace_focus`; non-Wide emits `MusicArtistActivate`), while retaining the filter-local artist expansion arm; verify key tests that unfiltered Enter emits the artist Hero intent without changing expansion and filtered Enter toggles locally without opening/focusing a Hero in Wide or non-Wide
- [x] 1.3 Add real `Application::tick()` coverage for Enter on an unfiltered artist root in Wide (artist Workspace focused, expansion unchanged, layout geometry unchanged) and non-Wide (Library Hero overlay open with the artist Workspace focused), plus filtered Wide and non-Wide cases proving local expansion and no Hero entry; verify the tree tick integration tests pass
- [x] 1.4 Confirm Right on a collapsed root still expands and Right on an expanded root still enters the Workspace, and that Left collapse is unchanged; verify the existing tree navigation tests plus one tick test per chord

## 2. Track rows carry their number

- [x] 2.1 Extract the Workspace track-row label (`"{number}. {title}"` with index fallback) into one shared helper and reuse it from `build_track_rows`/`track_row`; verify the track-row tests still pass
- [x] 2.2 Build the tree's `MusicTreeTrack` projection from that helper so track items show numbered titles; verify a `music_content`/tree test asserting a numbered track label and an index fallback for a track with no index number
- [x] 2.3 Verify the track-number label is what the tree paints (not just what the model holds) at the Wide and smallest non-Wide tree fixtures; verify the tree render characterization tests

## 3. Artist-Workspace playback spans the discography

- [x] 3.1 Add a shell path that resolves an artist's ordered in-scope tracks and chosen `EmbyItem` from the artist-detail cache, then sends the full flattened disc/track-ordered list with the chosen track's start index to the existing routed playback executor, reusing the queue-source behavior already used by Music track playback (no new `QueueSource` variant, persistence shape, or ctrl payload); verify with an `actions` unit test that preceding tracks remain queued while playback starts at the selected index
- [x] 3.2 Add the typed `MusicArtistTrackActivate` intent carrying only the artist target and stable track ID; verify `cargo check` and a handler test proving the shell resolves the item from its cache
- [x] 3.3 Emit that intent from every artist-Workspace activation path (keyboard Enter, `HeroActivate`, and Workspace row double-click), leaving all album-Workspace routes on the existing album path; verify with a `music_content` owner test per path, a Library Hero overlay activation test, and a tick integration test asserting the whole flattened discography reaches the routed executor from the selected index
- [x] 3.4 Verify the artist Workspace's single-click select and track movement behaviour is unchanged; verify the existing artist-workspace tests pass

## 4. Tree pointer gestures focus the panel

- [x] 4.1 Add the typed `LibraryPanelFocus` request for resolved tree gestures that have no other external effect and handle it by focusing Library; verify repeat selection, local expansion, childless-album claim, and boundary-clamped tree wheel with shell-message/owner tests
- [x] 4.2 Focus Library at the start of the existing `MusicAlbumCursor` handler and the new Music artist-track/play-now handlers; verify each handler without adding a second message from the owner
- [x] 4.3 Add a Music-specific context-menu request carrying the already-resolved targets/anchor whose shell handler focuses Library before opening the menu; keep generic `RowContextMenu` behavior unchanged for other destinations, and verify first click, toggle/range click, artist-root click, track double-click, and context-click owner cases
- [x] 4.4 Add a real tick test that clicking a tree row while Queue holds focus moves panel focus to Library; verify it passes and that a click on another panel still leaves Library unfocused

## 5. Tree double-click expands or plays

- [x] 5.1 Add the owner policy that a double-click reaches the owner instead of opening the Hero overlay, defaulting to the current hero-bearing behaviour and overridden false by Grouped Music; verify with a panel/owner policy unit test
- [x] 5.2 Rewrite the owner's tree double-click arm with explicit node-kind dispatch: toggle persistent expansion for an artist root or album leaf with cached children, claim a childless album leaf without state change, and emit the stable-ID track play-now intent for a track; route production local-filter pointer input through current-frame filtered tree geometry rather than the empty flat-result carrier, and verify all four node cases both filtered and unfiltered
- [x] 5.3 Add one grouped-track resolver used by tree Enter and play-now: autoload enabled resolves cached playable album tracks in disc/track order and the selected start index; disabled resolves only the selected track. Feed its complete `PendingQueueAction::PlayItems` into the existing playback/admission executor; verify both policies and resolution failure without queue mutation
- [x] 5.4 Add `ConfirmAction::ReplacePopulatedQueue` and its dispatcher/predicate. Empty local or direct-remote target queues execute immediately; populated queues store the action and prompt; confirmed dirty saved-playlist replacement proceeds through the existing save/discard prompt before execution; cancellation at either prompt changes neither queue nor playback. Verify empty/populated local and direct-remote queues plus populated+dirty save, discard, and cancel in `input_confirm_keys` tests
- [x] 5.5 Add real tick coverage: filtered and unfiltered double-click on artist/album nodes expands without a Hero in Wide and non-Wide; a childless album is claimed unchanged; tree-track Enter proves autoload enabled/disabled; Enter or double-click with an empty queue plays immediately; either with a populated queue opens confirmation and only plays after the complete confirmation sequence

## 6. Music "Go to Library" lands or reports

- [x] 6.1 Write a failing tick-level repro of the reported silent no-op using the user's shape (Grouped Music with a retained destination and a queued track) that pins the diagnosed root cause recorded in design D7: `NavigateLanding::Album` rebuilds its path from raw Emby ancestor depth rather than the configured `music.levels` album shape, so no Music owner is eligible and the projection returns early with no fallback painter or error; verify the repro fails on the current silent path before the fix
- [x] 6.2 Build the grouped Music landing completely before commit, then atomically apply its tab, nav stack, saved Library position, and retained-owner re-anchor; fix the diagnosed silent path and verify 6.1 now lands with the selected track while existing item-navigation tests still pass
- [x] 6.3 Add regression coverage that navigation into a previously mounted tree re-anchors to the resolved album and selects the navigated track in the Workspace; verify the new tick test passes
- [x] 6.4 Inject an apply-stage failure after album resolution and verify the existing library-error feedback appears while active tab, nav stack, saved Library position, and retained component selection remain byte-for-byte/structurally unchanged

## 7. Verification

- [x] 7.1 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`; verify both are clean
- [x] 7.2 Run `cargo nextest run -p mbv -p mbv-core` and compare against the pre-change baseline; verify no regressions
- [x] 7.3 Run `openspec validate --all` and sync the applied deltas into `openspec/specs/` when the change completes; verify validation is clean
- [x] 7.4 Manual PoC check at Wide, non-Wide, Mini, and Library Hero overlay states covering Enter's artist Hero and tree-track playback/autoload/confirmation behavior, numbered track rows, discography playback, click focus, double-click expand/play, and Go to Library
