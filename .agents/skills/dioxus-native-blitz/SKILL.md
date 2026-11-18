---
name: dioxus-native-blitz
description: Work on the mbv GUI Client — the Dioxus Native window over the Blitz rendering engine in crates/mbv-gui. Use when the user mentions the GUI, mbv-gui, the window, a GUI surface, Dioxus, Blitz, rsx!, components, signals, or GUI styling or input. Triggers include "add a GUI panel", "the window paints X wrong", "how does rsx work", "make a component", "the signal does not update", "a Blitz gap", "regenerate the stylesheet", "make dev", "add a GUI surface to the Client". Do not use for TUI work (mbv-visual-tweak owns TUI appearance tweaks) or for changes outside crates/mbv-gui.
---

# Dioxus Native on Blitz — the mbv GUI Client

The GUI Client is `crates/mbv-gui`: a Rust window built with Dioxus Native on
Blitz. It has no webview, no JavaScript, and no Node tool. This skill gives the
framework knowledge an agent needs: the syntax, the reactivity, the events, the
styling build, and the engine gaps. It replaces a web search for the common
questions.

## Read first

1. `docs/architecture/gui-client.md`. It owns the GUI contract: the process
   shape, the bridge, the launch, the surface-adding steps, the tests, and the
   full Blitz-gap table.
2. The ADRs: `docs/adr/0039` (why Dioxus Native on Blitz), `0036` and `0037`
   (the window owns its interaction state), `0038` (change-driven sections),
   `0040` (a key's target follows the pointer), `0041` (one Queue entry).
3. `openspec/specs/gui-client/spec.md` for the window's current contract.

The GUI does not keep parity with the TUI. The design canvas is the reference.
Do not copy a TUI convention into the window.

## Upstream references

- <https://github.com/DioxusLabs/dioxus/blob/main/AGENTS.md> — the Dioxus
  workspace map, the component pattern, and a table of deeper notes under
  `notes/architecture/` in that repo (`01-CORE.md` for the VirtualDOM,
  `03-RSX.md` for the macro, `04-SIGNALS.md` for reactivity,
  `06-RENDERERS.md` for how each renderer works, `07-HOTRELOAD.md`,
  `08-ASSETS.md` for `manganis`).
- <https://github.com/DioxusLabs/blitz/blob/main/AGENTS.md> — the Blitz
  engine's build needs, its test signals, and its engine gotchas. Read it
  before you judge a Blitz behaviour a bug.

Both live on upstream `main`, which moves. This repo pins pre-release
versions, so the API authority is the pinned crate source in the local Cargo
registry: `~/.cargo/registry/src/*/dioxus-native-0.8.0-alpha.3/`,
`blitz-dom-0.3.0-beta.4/`, and the rest. Read that source before you trust a
web page about another version.

## The pinned stack

| Crate | Pin | Role |
| --- | --- | --- |
| `dioxus-native` | `=0.8.0-alpha.3` | the renderer adapter: the prelude, `use_window`, a `winit` re-export, `DioxusDocument`, `DocumentConfig` |
| `blitz-shell`, `blitz-traits`, `blitz-dom` | `=0.3.0-beta.4` | the window shell, the net traits, the DOM |
| `parley` | `=0.12.0` | text measurement |
| `blitz-paint`, `anyrender` | `=0.3.0-beta.4`, `=0.14.0` | paint commands (dev-dependencies the frame tests read) |

The pins are exact, by design (ADR 0039). Cargo must keep one copy of
`blitz-dom` and `parley`: a crate that needs one of them pins the same
version.

Blitz is Stylo (Servo's CSS engine) + Taffy (layout) + Parley (text) +
wgpu paint through `vello-hybrid`, on winit windows. What the CSS and the DOM
do here is what Stylo, Taffy, and `blitz-dom` implement — not what a browser
spec says. The upstream Blitz AGENTS.md names the gaps WPT covers and the
quarantines.

The window runs in one process with the core thread. Values cross an
in-process channel, not a wire. There is no hot reload: a code change takes a
restart of the program. Do not reach for the `dx` CLI; this repo does not use
it.

## The mental model

- A component is a Rust function that returns an `Element`. `Element` is
  `Option<VNode>`; the `rsx!` macro builds the node.
- Dioxus diffs the VirtualDOM. Blitz applies the diff to a real DOM tree and
  paints it.
- A component re-runs when a signal it read during its render is written.
  Nothing else re-runs.
- `Signal<T>` is a `Copy` handle to reactive storage. Copy the handle freely;
  clone the value when you need the value.
- An effect runs after the render and re-runs when a signal its body reads
  changes.

## rsx! essentials

```rust
use dioxus_native::prelude::*;

#[component]
fn QueueColumn(rows: Vec<Row>, title: String, on_press: Callback<Row>) -> Element {
    let mut hovered = use_signal(|| false);

    rsx! {
        div {
            class: "flex h-full min-h-0 flex-col flex-1",
            id: "queue-column",
            tabindex: 0,
            // Quote any attribute the macro does not model.
            "data-queue-column": "",
            // Inline CSS: a format string with { } holes.
            style: if hovered.cloned() { "border: 1px solid var(--edge);" } else { "" },

            // Text: one literal with { } holes.
            "Now playing: {title}"

            // Children nest as braces. A component call names its props.
            PlayButton { on_press: on_press.clone(), label: "Play" }

            // `if` renders nothing when it is false.
            // `if let` and `match` arms work the same way.
            if let Some(frame) = overlay {
                ConfirmOverlay { message: frame.message }
            }

            // Lists: `for` over an iterator. Key every row.
            for row in rows.iter() {
                div {
                    key: "{row.slot}",
                    "data-row": "{row.slot}",
                    onclick: move |event: MouseEvent| {
                        event.prevent_default();
                        on_press.call(row.clone());
                    },
                    "{row.title}"
                }
            }
        }
    }
}
```

Rules the compiler teaches you slowly:

- Elements are lowercase (`div`, `button`, `main`); components are PascalCase
  (`PlayButton { .. }`). No self-closing syntax: every element is
  `name { .. }`.
- Known attributes are bare words (`class`, `id`, `tabindex`); everything
  else is a quoted string name (`"type"`, `"aria-label"`, `"data-row"`).
- `rsx! {}` is the empty fragment. Return it to render nothing.
- Handlers must be `move` closures: they own their captures.
- The prelude (`use dioxus_native::prelude::*`) exports `rsx`, `component`,
  `Props`, `Element`, `Callback`, the hooks, the signals, the stores,
  `provide_context`, `consume_context`, `spawn`, `Event`, the `html` element
  vocabulary, and the event types. `use_window` is a separate import from
  `dioxus_native`, and so is `dioxus_native::winit`.

## Components and props

Two property styles exist. This repo uses the explicit struct for window
components, because a `PartialEq` struct memoizes renders:

```rust
#[derive(Props, Clone, PartialEq)]
pub struct PanelDividerProps {
    pub side: DividerSide,
    pub width: f32,
    pub on_drag: Callback<f32>,   // the child calls it: on_drag.call(width)
    pub on_reset: Callback<()>,
    pub children: Element,        // a slot for child rsx
}

#[component]
pub fn PanelDivider(props: PanelDividerProps) -> Element { /* .. */ }
```

- Write a custom `Debug` for a props struct that holds callbacks: the callbacks
  carry no painted fact. Do not derive `Debug` over them.
- Callback props follow the `on_` prefix, and the caller passes them by name at
  the call site.
- A test component may take implicit props instead: `fn QueueColumn(children:
  Element) -> Element`.
- A component function must be PascalCase. The macro generates the component
  type from it.

## Signals, effects, and context

```rust
// Create.
let mut hovered = use_signal(|| false);     // per-component reactive state
let scale = Signal::new(1.0);               // anywhere; a Copy handle
let sections = use_hook(Sections::new);    // once per component instance

// Read (a render that reads re-runs on write).
let value = *hovered.read();                // borrows the value
let value = hovered.cloned();               // a copy of the value

// Read without subscribing: handlers, effects, peek-only facts.
let value = hovered.peek();

// Write.
hovered.set(true);                          // for Copy values
mounted.write().retain(|k, _| *k < count);   // &mut T for the rest
```

Hard rules:

- Every signal write marks its subscribers — even a write with an equal
  value. An effect that reads a store it then writes re-queues itself
  forever. Read one store, write a different one.
- Write one batch in one task poll. Dioxus renders the batch once, and no
  frame shows half a batch.
- `peek` in a callback when a fact must re-render nothing. The pointer
  target works this way (ADR 0040): the launch copy writes it, key handlers
  peek it, no component subscribes to it.
- `use_hook` runs once per component instance and keeps its value across
  re-renders. Hold stores and context values there, not in `use_signal`.

```rust
// Context: the root provides, descendants consume by type.
provide_context(sections.clone());
let link = consume_context::<CoreLink>();
let epoch = try_consume_context::<LayoutEpoch>();

// Effects: run after render; track the signals the body reads.
use_effect(move || { sync_link.sync(); });

// Teardown on unmount.
use_drop(move || { drop_drag.end(); });

// Async: spawn one task, or run one long-lived future.
spawn(async move { event.set_focus(true).await; });
use_future(move || async move {
    while let Some(event) = events.next().await { drain.dispatch(event); }
});
```

The window has no `setTimeout` and no `requestAnimationFrame`.
`src/window/clock.rs` provides both as a `Clock`: a timer thread with a
deadline queue, and a frame gate on presented frames. Use it; never start a
frame loop that repaints on vsync (it held the main thread near half a core).

## Events

| Handler | Event type | Reads |
| --- | --- | --- |
| `onkeydown` | `KeyboardEvent` | `event.key()`, `event.code()` |
| `onpointerdown`, `onpointerup`, `onpointermove`, `onpointerleave`, `onpointercancel` | `PointerEvent` | `event.data.trigger_button()` (`Option<MouseButton>`), `event.data.client_coordinates()` |
| `onwheel` | `WheelEvent` | `event.data` carries the deltas |
| `onclick`, `ondoubleclick`, `oncontextmenu` | `MouseEvent` | pointer facts through `event.data` |
| `oninput` | `FormEvent` / `Event<FormData>` | `event.value()` |
| `onfocus`, `onfocusin`, `onblur` | `FocusEvent` | focus bookkeeping |
| `onmounted` | `Event<MountedData>` | `event.set_focus(true).await`; the mounted event answers an async client-rect read |

Rules:

- `event.prevent_default()` stops the browser default. `stop_propagation()`
  stops the parent handlers. A key handler that consumes a key calls
  `stop_propagation` before the root's forwarder sees it.
- `dioxus-native-dom` panics on some DOM event kinds (drag, clipboard, media,
  and others). No handler uses them. `src/window/input.rs` owns the window's
  event vocabulary — add a kind there, never a handler elsewhere.
- Blitz clears the DOM focus on a click that matches no text input, checkbox,
  radio, summary, label, or link, and it drops focus with a removed node. The
  root element carries the id `mbv-root`, and the launch copy's
  `restore_root_focus` gives the keyboard back. A surface that needs keys
  keeps that model.
- The root holds the one `keydown` handler; a key with nothing focused falls
  back to `<html>` and reaches no handler. That is why the root takes focus on
  mount.

## Styling

Styling is Tailwind v4 class strings plus a few CSS variables. The generated
stylesheet is compiled ahead of the build and committed.

- A `class:` string holds the utilities. State looks ride data attributes:
  `data-[divider-dragging]:bg-[var(--canvas-border)]`.
- Theme colours resolve through CSS variables: `bg-[var(--canvas-queue)]`,
  never a literal colour. `src/window/theme.rs` maps the theme roles to
  variables.
- Inline styles are format strings: `style: "width: {w}px; margin-inline:
  -{w / 2.0}px;"`.
- The stylesheet mounts as a `style` element whose text child is the CSS
  (`src/window/app.rs::root_style_element`). Never use `dangerous_inner_html`:
  the default `DocumentConfig` leaves Blitz's HTML parser provider as a no-op,
  so inner HTML writes no text at all.
- `assets/app.css` imports Tailwind and the vendored shadcn and tw-animate
  files, scans `src/window`, and declares the `@font-face` rules.
  `make gui-css` regenerates `assets/app.generated.css` with the standalone
  Tailwind CLI v4.3.5 (SHA-256
  `e47695cee3f0d90623a91f3939b08bdee610eb5ad99340adff9d4145b284d6cc`, on
  `PATH` as `tailwindcss`, or `TAILWINDCSS=/path`). A class added to the Rust
  sources needs `make gui-css` before its commit. The generated file is
  committed; CI does not check it; no Node tool runs in a normal build.
- Icons are Lucide only (`src/window/icons.rs`).

Blitz does not paint `text-overflow`. Use the `Truncate` component
(`src/window/truncate.rs`): it measures the text with the bundled faces and
writes a cut that ends in `…`. Pass the `TextStyle` of the box's class string
(family, size, weight, letter spacing). Keep the `truncate` class on the box so
it still clips. A box with no measured width clips its whole text; it never
paints the ellipsis alone. A first measure sets a window-wide re-measure that
the next presented frame resolves.

## Blitz gaps you will hit

| Gap | Handling |
| --- | --- |
| No `text-overflow: ellipsis` | `Truncate` measures and cuts (`src/window/truncate.rs`) |
| No `ResizeObserver` | `onmounted` events and the layout epoch; a presented frame's change fact bumps the epoch once (`WindowEvent::Frame`) |
| No net-handler error callback | the net provider sends `ArtEvent::Failed`; the window retries with a capped backoff (1 s to 30 s) |
| Some DOM event kinds panic | no handler for them; `src/window/input.rs` owns the vocabulary |
| Animations and transforms repaint the whole window | accepted; they run only while shown |
| No GPU | surface creation panics; the packages depend on the Vulkan loader; accepted |
| Slow Wayland resize, surface timeouts | accepted |
| No window content reload | restart the program for a change |

`docs/architecture/gui-client.md` holds the full table and owns it. Add a row
there when you meet a new gap; work around it in this repo's code; never
patch Blitz in-tree. A fix for Taffy, Parley, anyrender, or Stylo belongs
upstream in its own repo.

## The window's mbv wiring

A short map; `docs/architecture/gui-client.md` owns the detail.

- One process: the core thread (`src/core_thread.rs`) runs the Client core
  tick; the window (`src/window/`) holds the interaction state and paints.
- The channel: the core sends `WindowEvent::Batch`, `Timer`, `Frame`, `Art`,
  `Viewport`, `Quit`; the root's drain task dispatches them; `CoreLink`
  carries the window's typed inputs back. No serde, no commands, no events
  through the DOM.
- Sections: one signal per section (`src/window/sections.rs`), written only
  when the batch's revision differs from the held one.
- The root (`src/window/app.rs`) provides the contexts: `Sections`, the
  `Clock`, the `LayoutEpoch`, `ArtEvents`, `PanelWidthsStore`, `DividerDrag`,
  the pointer target, `ViewportSize`, `CoreLink`.
- The launch copy (`src/window/launch.rs`) is a vendored copy of
  `dioxus-native`'s `launch_cfg_with_props`, marked `This crate:` at every
  difference. The pinned crate source is the authority for it.
- The net provider (`src/window/net.rs`) serves the `mbv-art` scheme from the
  image cache and serves the bundled fonts.
- Tokio is edge-only here too: `dioxus-native` enables tokio for its asset
  resolver, and no mbv code may use a tokio API. Use `spawn`, `use_future`,
  and the `Clock`.

## Commands

| What | Command |
| --- | --- |
| Build and run both binaries | `make dev` (or `make dev-release`) |
| Regenerate the stylesheet | `make gui-css` |
| Check | `cargo check -p mbv-gui` |
| Test | `cargo nextest run -p mbv-gui` |
| Logs | `mbv-gui.log` in the mbv state directory |

`make dev` builds `mbv` and `mbv-gui` together, because the handshake refuses
an Owner whose build id differs from the window's. `cargo run -p mbv-gui`
alone leaves the Owner stale after a commit.

## Tests

- Window tests sit beside their modules: `src/window/**/tests.rs`. Each test
  builds its input, calls once, and asserts on the return or the painted
  output. No test opens a window, sleeps, or reaches an Owner or a network.
- A component test renders a fixture batch through a `DioxusDocument` and
  asserts the painted structure, not the appearance. An SVG assertion reads
  the parsed `usvg` tree; a frame assertion records the `anyrender` paint
  commands and reads their geometry.
- The crate's public contracts sit in `crates/mbv-gui/tests/` with the
  channel, the batch sink, and the Owner steps mocked.
- Live behaviour in the real window is the user's manual check. Ask before
  you run anything live on the user machine.
