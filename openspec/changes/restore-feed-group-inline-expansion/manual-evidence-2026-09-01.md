# Manual / harness evidence — 2026-09-01

A real configured environment was exercised through tmux using the built binary and the user's configured file:

```text
XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles ./target/debug/mbv
```

Commands used:

```text
rtk cargo build -q
XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles tmux new-session -d -s feed-evidence -x 60 -y 20 './target/debug/mbv'
tmux capture-pane -pt feed-evidence
tmux resize-window -t feed-evidence -x 100 -y 30
tmux capture-pane -pt feed-evidence
tmux resize-window -t feed-evidence -x 140 -y 40
tmux capture-pane -pt feed-evidence
```

Results: the process started and rendered live configured Emby/feed-derived content at all three sizes. Captures showed the normal Home surface (Emby/YouTube content), not the Feeds tab. Attempts to select Feeds by sending `]` did not change the selected surface; the tab bar remained on Home. A temporary `~/.local/state/mbv/prefs.json` containing `{"library_tab":8}` was restored after the run, but startup still rendered Home. Therefore no genuine focused-Feeds observations are claimed for Narrow selected-row expansion/banner completeness, group/watched-pill interaction, bottom-edge scrolling, or Wide no-conflicting-expansion.

A repeat run against the effective user config confirmed `/home/slatkin/Dev/dotfiles/mbv/config.toml` exists and contains four `[[feeds]]` tables with non-empty URLs (metadata only; URLs and credentials were not printed). `XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles` was used, so this file is the effective config path.

Commands used:

```text
XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles tmux new-session -d -s feed-evidence -x 60 -y 20 './target/debug/mbv'
tmux send-keys -t feed-evidence Escape
tmux send-keys -t feed-evidence BTab
tmux resize-window -t feed-evidence -x 100 -y 30
tmux resize-window -t feed-evidence -x 140 -y 40
```

Results: after dismissing the startup overlay and sending BTab, the Feeds destination was selectable. At 60x20 and 100x30, the narrow tab strip clipped before displaying the Feeds label, while the selected feed content rendered (subscription pills, All/Played/Unplayed filter, Recent group, and an entry). At 140x40, the tab strip visibly included `FEEDS`, and the feed surface rendered Recent and Older than two weeks groups with entries and durations. No credentials or feed URLs were exposed. The tmux session was terminated after capture.

No production or test files were changed.

## Task 5.4 follow-up evidence

The requested real-feed run was repeated from the built binary at all three sizes. The effective configuration was passed exactly as `XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles`; the startup overlay was dismissed with `Escape`, and `BTab` selected Feeds. Exact commands:

```text
rtk cargo build -q
XDG_CONFIG_HOME=/home/slatkin/Dev/dotfiles tmux new-session -d -s feed-evidence -x 60 -y 20 './target/debug/mbv'
tmux send-keys -t feed-evidence Escape
sleep 1
tmux send-keys -t feed-evidence BTab
sleep 2
tmux resize-window -t feed-evidence -x 100 -y 30
tmux resize-window -t feed-evidence -x 140 -y 40
tmux capture-pane -pt feed-evidence -S -60
tmux kill-session -t feed-evidence
```

Observed, with no claims beyond the captures:

* **60x20:** the Feeds content showed the subscription pills, `All · Played · Unplayed`, `Recent`, a `▁` separator, and one selected entry (`Patron's Choice for August 2026: Murder Was the Case …`) with date `2026-08-28`. No expanded overview/banner text was visible in this real entry capture; the selected row remained a single visible content row.
* **100x30:** the Feeds content showed `Recent`, the selected entry and its duration/progress, followed by additional entries. The capture contained the same `▁`/`▔` framing and no separately identifiable overview banner. The tab strip was clipped before the `FEEDS` label.
* **140x40 (Wide):** the tab strip visibly included `FEEDS`. The Feeds surface showed `Recent` and `Older than two weeks` groups with entries and durations. The selected media presentation remained in the left rail while the feed list occupied the right rail; no conflicting selected-row expansion was observed in Wide.
* **Group/watched interaction:** a direct mouse click was not performed because this tmux session did not provide a verified mouse-event path. The supported key interaction `w` was sent while Feeds was focused; the capture changed the filter area to a loading state with `Press r to load feeds`, so no successful filter-state transition is claimed. `Tab` + `Enter` subsequently moved to Home rather than selecting a feed group, so no successful group selection is claimed.
* **Bottom-edge scrolling:** `PageDown` was sent three times after the above interaction sequence, but the surface had already left the focused Feeds state; therefore no bottom-edge scroll result is claimed. The real-feed run does not establish complete expansion, selector activation, or bottom-edge scrolling behavior.

The exact limitation is that this environment's live feed data/capture did not expose a metadata-bearing expanded selected row, and the attempted key interactions triggered loading/tab navigation rather than a confirmed selector change. No invented success claim is added.

Supplementary focused automated check:

```text
rtk cargo nextest run -p mbv feed_home_video_group --no-fail-fast
```

Result: PASS (10 passed, 1234 skipped). This covers the existing fixture assertions for 60x20 Narrow and 140x40 Wide plus metadata/banner, pills, row geometry, and scroll projection; it does not provide real-service manual evidence and does not cover 100x30.
