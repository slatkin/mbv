# Design

## Context

Two panels paint the same transport arrangement and each keeps its own retained hit geometry:
`QueuePlaybackPanel` for the queue column band, `LibraryPlaybackPanel` for the right-column strip
(`openspec/specs/queue-playback-panel`). Both route a pointer event to `Msg::Playback(...)` through
their own `mouse()`, and the shell turns `PlaybackRequest::SeekTo(fraction)` into
`App::seek_to_fraction`.

Both currently match the raw event kind:

```rust
MouseEventKind::Down(MouseButton::Left)
    if self.seekbar_area.contains(point) && self.seekbar_area.width > 0 =>
```

Neither holds a `MouseGestureState`, so neither can tell a single press from a double-click, and
neither sees a drag. The shared recognizer
(`crates/mbv-components/src/mouse/gesture.rs`) already provides both: a private `MouseGestureState`
per parent, the 400 ms window, exact-position matching, and `Drag { from, to }` per motion event.

The mouse-input spec already requires a mounted parent to recognize gestures through a private
`MouseGestureState`, and requires drag recognition to report the anchor and the current position.

## Goals / Non-Goals

**Goals:**

- A seekbar seek needs a double-click, in both panels.
- A drag that anchored inside the bar scrubs the bar.
- The transport controls keep their current behaviour: one press, one intent.

**Non-Goals:**

- No change to the fraction arithmetic, the retained hit geometry, the availability gates, the
  wheel, or right-click.
- No seek preview, no sub-cell precision, no scrub outside the bar span.
- No new shared "transport pointer" module: the two panels are separate components with separate
  retained geometry, and each keeps its own recognizer state and its own test.

## Decisions

### D1: Each panel feeds every mouse event to its own `MouseGestureState`

The recognizer must see the first press of a pair, or the second press cannot be recognized as a
double-click. So `mouse()` calls `self.mouse_gestures.recognize(event)` before it matches anything,
including events it will ignore.

The recognizer also arms its drag anchor on the same press, so the drag gesture is available to the
scrub arm without a second piece of local state.

### D2: The seekbar arm requires `MouseGesture::DoubleClick`

The seekbar arm keeps its containment and width guards and adds the recognized double-click. A
single press on the bar produces `MouseGesture::Click`, which matches no arm and resolves nothing.

This is why the transport arms stay on the raw press kind: a second press inside the 400 ms window
produces `DoubleClick` rather than `Click`, so a gesture-driven button arm would silently drop the
second press and change two toggles into one. The buttons are not in scope, and the raw kind keeps
them bit-for-bit as they are.

### D3: A scrub needs an in-bar anchor and a throttled admission

The scrub arm takes `MouseGesture::Drag { from, to }` and admits it only when **both** `from` and
`to` are inside the bar span. The anchor test is the part that makes the gesture a scrub of this
bar: a drag that started elsewhere (a queue-row drag that wanders across the band) has an anchor
outside the bar and must not seek. The old code tested only the current point, so that cross-surface
drag could seek.

The throttle is a panel field `scrub_at: Option<Instant>`, written only when a scrub is admitted,
and the interval is 150 ms (the legacy cadence). `None` admits the first scrub immediately — the
legacy field started at `Instant::now()`, which swallowed the first 150 ms of a scrub for no reason.

A test injects the outcome rather than the delay: it sets `scrub_at` to a past instant or clears it,
and never sleeps.

### D4: Both panels carry the same rule, and one test each

The panels are separate production code with separate retained geometry, so a regression in one is
not caught by the other's test. Each panel extends its existing seekbar test with the double-click
gate (that test already owns "the seekbar resolves a seek against the retained geometry") and adds
one test for the scrub admission rule (anchor and throttle). No other test is added: the tick
integration fixtures keep owning delivery and projection, and they only replace their single-click
fixture with a double-click fixture.

### D5: Repeated clicks inside the window stay semantic

A third press at the same position inside a still-open window is another `DoubleClick`, so it seeks
again, exactly as the legacy `is_double` rule did. The spec states the gesture, not a click count.

## Risks / Trade-offs

- [The two presses of the first double-click no longer seek on the press that opens the window] →
  Intended: that is the restored behaviour. A user who wants a jump uses the double-click; a stray
  click no longer moves playback.
- [A double-click straddling two ticks] → The recognizer is per-event with wall-clock timestamps,
  not per frame, so a click in one tick and its partner in the next inside 400 ms still seek. The
  tick-integration fixtures cover delivery across two steps.
- [The throttle drops a scrub motion] → Intended; the next admitted motion inside the interval
  carries the newer column, and the fraction is recomputed from the current pointer, so the last
  mouse position before release is not lost for longer than the interval.
- [A drag leaves the bar and returns] → Motions outside the span are dropped, and the next in-bar
  motion resumes the scrub from the current column.
