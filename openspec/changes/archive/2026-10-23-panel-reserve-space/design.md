# Design

## Context

`src/pin.rs` is the only pinwin caller. It picks the reserve mode in two places: `run_argv`
hardcodes `--reserve push` at launch, and `pinned_width_argv` maps `Collapsed` to `Push` and
`Expanded` to `Cover` on every `set-size` request. pinwin's `cover` keeps the gap the last
pushing setting reserved (`HeldGap::after_publish`), which is what lets expansion cover tiles
without moving them; a covering start holds an empty strip (`HeldGap::start`). `set-reserve` exists since pinwin
0.4.1, mbv's stated minimum.

## Goals / Non-Goals

**Goals:**

- `[panel] reserve` picks the collapsed panel's mode: `push` when true (default), `cover`
  when false; the expanded width always covers.
- The F2 `Reserve space` row behaves like the other live Panel rows while pinned.

**Non-Goals:**

- Releasing an already-held strip on a running panel. pinwin has no request for that;
  slatkin/pinwin#59 tracks it. When it lands, only the argv the `Reserve space` row builds
  changes.
- Any pinwin change, and any expanded-width mode choice.

## Decisions

- **D1: The row applies live via `set-reserve`, not launch-only.** Alternative considered:
  treating the row like the accent rows (save, announce "applies on next launch", send no
  request). Rejected because it would be a half-truth in both directions: the collapse
  `set-size` requests read the live config anyway, so a launch-only row could not stay
  launch-only without snapshotting the launch-time mode — and enabling *can* honestly apply
  live, since `set-reserve push` reserves the strip at the current width. Disabling sends
  `set-reserve cover`: the panel stops pushing but keeps the strip it already holds, which is
  why the disable path adds the next-launch toast. One honest row, one new request variant.
- **D2: `pinned_width_argv` reads `reserve` from the config it is already handed.**
  Collapsed maps to `Push` when `reserve` is true, `Cover` when false. This keeps every
  collapsed `set-size` consistent with the option mid-session: with `reserve` off a collapse
  never re-reserves, and with `reserve` toggled on mid-session a later collapse pushes as the
  option says.
- **D3: The disable toast lives on the accepted outcome, not the request.** Only a `cover`
  the panel accepted announces the held strip; a refusal already flashes pinwin's reason.
  Enabling needs no toast: the strip visibly appears.
- **D4: While expanded, `Reserve space` saves and sends no request.** `set-reserve push` pushes
  at the panel's current size, so sending it at the expanded width would reserve the full
  expanded strip and move every tiled window — contradicting "expanded always covers". The
  row takes the accent rows' no-request path instead (`pinned_setting_request` returns `None`;
  `apply_panel_setting_outcome` saves and announces). The saved value takes effect on the next
  collapse because D2's argv reads the live config. Each direction still gets a neutral toast:
  an enable says the strip is reserved on the next collapse, and a disable says the strip the
  panel already holds is released on the next `mbv --pin` launch.

## Risks / Trade-offs

- Disabling while pinned looks like a no-op (tiles stay pushed) until relaunch → the neutral
  toast names the limitation, and pinwin#59 is the follow-up.
- `set-reserve` is refused by pinwin older than 0.4.1 → same failure path as every pinned
  request: warning toast, nothing saved; the documented minimum is 0.4.1.

## Migration Plan

None: `reserve` defaults to `true`, which is today's behavior bit for bit.
