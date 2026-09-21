# Tasks

## 1. Artist-root Enter opens its Hero

- [ ] 1.1 Make the Library panel's `hero_overlay_enter_available` report true for a Grouped Music artist root only while the tree filter is inactive, so non-Wide Enter reaches the overlay path without bypassing filtered interaction; verify with panel/owner tests for unfiltered artist eligibility, filtered artist ineligibility, and unchanged album-leaf behavior
- [ ] 1.2 Replace the unfiltered `MusicContent` Enter-on-artist expansion arm with the Right chord's Hero entry (Wide takes `enter_artist_workspace_focus`; non-Wide emits `MusicArtistActivate`), while retaining the filter-local artist expansion arm; verify key tests that unfiltered Enter emits the artist Hero intent without changing expansion and filtered Enter toggles locally without opening/focusing a Hero in Wide or non-Wide
- [ ] 1.3 Add real `Application::tick()` coverage for Enter on an unfiltered artist root in Wide (artist Workspace focused, expansion unchanged, layout geometry unchanged) and non-Wide (Library Hero overlay open with the artist Workspace focused), plus filtered Wide and non-Wide cases proving local expansion and no Hero entry; verify the tree tick integration tests pass
- [ ] 1.4 Confirm Right on a collapsed root still expands and Right on an expanded root still enters the Workspace, and that Left collapse is unchanged; verify the existing tree navigation tests plus one tick test per chord

## 2. Track rows carry their number

- [ ] 2.1 Extract the Workspace track-row label (`"{number}. {title}"` with index fallback) into one shared helper and reuse it from `build_track_rows`/`track_row`; verify the track-row tests still pass
- [ ] 2.2 Build the tree's `MusicTreeTrack` projection from that helper so track items show numbered titles; verify a `music_content`/tree test asserting a numbered track label and an index fallback for a track with no index number
- [ ] 2.3 Verify the track-number label is what the tree paints (not just what the model holds) at the Wide and smallest non-Wide tree fixtures; verify the tree render characterization tests

## 3. Artist-Workspace playback spans the discography

- [ ] 3.1 Add `QueueSource::Artist { name: String }`, keep every existing match exhaustive, and verify queue-state serialization round-trips every old variant unchanged plus the new variant; document in the test that downgrade reading of the new variant is unsupported
- [ ] 3.2 Add a shell path that resolves an artist's ordered in-scope tracks and chosen `EmbyItem` from the artist-detail cache, then sends the full flattened disc/track-ordered list with the chosen track's start index and artist queue source to the existing routed playback executor; verify with an `actions` unit test that preceding tracks remain queued while playback starts at the selected index
- [ ] 3.3 Add the typed `MusicArtistTrackActivate` intent carrying only the artist target and stable track ID; verify `cargo check` and a handler test proving the shell resolves the item from its cache
- [ ] 3.4 Emit that intent from every artist-Workspace activation path (keyboard Enter, `HeroActivate`, and Workspace row double-click), leaving all album-Workspace routes on the existing album path; verify with a `music_content` owner test per path, a Library Hero overlay activation test, and a tick integration test asserting an artist queue source
- [ ] 3.5 Verify the artist Workspace's single-click select and track movement behaviour is unchanged; verify the existing artist-workspace tests pass

## 4. Tree pointer gestures focus the panel

- [ ] 4.1 Add the typed Library-panel-focus request and handle it by focusing the Library panel; verify with a shell-message unit test
- [ ] 4.2 Focus the Library panel from the artist-track request handler as well, so a selection change to an artist root also focuses; verify the handler test
- [ ] 4.3 Make every tree gesture that resolves an artist, album, or track row request Library focus: selection arms emit their focus-carrying selection intent or the focus request (click, toggle-click, range-click, wheel), while context-click and double-click handlers focus as part of dispatching their resolved target; verify first click, repeat click, artist-root click, track double-click, and context-click owner cases
- [ ] 4.4 Add a real tick test that clicking a tree row while Queue holds focus moves panel focus to Library; verify it passes and that a click on another panel still leaves Library unfocused

## 5. Tree double-click expands or plays

- [ ] 5.1 Add the owner policy that a double-click reaches the owner instead of opening the Hero overlay, defaulting to the current hero-bearing behaviour and overridden false by Grouped Music; verify with a panel/owner policy unit test
- [ ] 5.2 Make the owner's tree double-click arm toggle expansion for an artist root or an album leaf with cached track children, claim a childless album leaf without changing state, and emit the stable-ID track play-now intent for a track item; verify with an owner test for all four cases
- [ ] 5.3 Add the play-now intent and the "replace a populated target queue" confirmation action backed by the existing pending-queue-action machinery; after the gate, call the same album-track resolver and routed playback executor as Enter. Verify empty/populated local and directly controlled remote target queues, plus `input_confirm_keys` tests that confirm executes and cancel drops pending playback
- [ ] 5.4 Add real tick coverage: double-click an artist root and an album leaf with cached children toggles expansion without opening a Hero in Wide and non-Wide; a childless album leaf is claimed without state change; a track with an empty target queue plays immediately; a track with a populated target queue opens confirmation and only plays on confirm; verify the tick integration tests pass

## 6. Music "Go to Library" lands or reports

- [ ] 6.1 Write a failing tick-level repro of the reported silent no-op using the user's shape (Grouped Music with a retained destination and a queued track), temporarily instrument each resolve/activate/apply stage, record the root cause in this change's design, and remove all temporary instrumentation before completing the task; verify the repro fails as reported before any fix and remains as the durable regression check
- [ ] 6.2 Fix the diagnosed silent path so the navigation lands on the album with the track selected, or surfaces the existing library-error feedback when it cannot; verify 6.1's repro now passes and the existing item-navigation tests still pass
- [ ] 6.3 Add regression coverage that a Grouped Music navigation into a previously mounted tree re-anchors to the resolved album and selects the navigated track in the Workspace; verify the new tick test passes

## 7. Verification

- [ ] 7.1 Run `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`; verify both are clean
- [ ] 7.2 Run `cargo nextest run -p mbv -p mbv-core` and compare against the pre-change baseline; verify no regressions
- [ ] 7.3 Run `openspec validate --all` and sync the applied deltas into `openspec/specs/` when the change completes; verify validation is clean
- [ ] 7.4 Manual PoC check at Wide, non-Wide, Mini, and Library Hero overlay states covering Enters artist Hero, numbered track rows, discography playback, click focus, double-click expand/play, and Go to Library
