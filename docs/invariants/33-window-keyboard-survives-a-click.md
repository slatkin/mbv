# Invariant 33 — A click never costs the window its keyboard

**Scope:** the window's root element and its DOM focus (`crates/mbv-gui/src/window/app.rs`,
`ROOT_ELEMENT_ID`), the focus repair (`crates/mbv-gui/src/window/launch.rs`,
`restore_root_focus`, `focus_within`), and the press path that runs it
(`MbvApplication::window_event`). Landed by the `replace-tauri-with-blitz` change.

## The invariant

The document's DOM focus SHALL sit inside the window's root element (the `main` that carries the
window's one `keydown` handler) whenever no field of the window holds it. A click, a removed field,
or any other focus change SHALL NOT leave the focus on the document's `<html>` element.

## Why it matters

The shell delivers a keydown to the document's focused node, and falls back to the document's root
element — the `<html>` element that `blitz-dom` creates. That element carries no Dioxus id, so
`DioxusEventHandler::handle_event` finds no element to dispatch to and returns: the event reaches
no handler. The window's only `keydown` handler is the root's, and only DOM focus connects the two.

`blitz-dom`'s click default action clears the DOM focus after a click that matches no text input,
checkbox, radio, summary, label or link (`events/pointer.rs`, `handle_click`'s
`if !matched { clear_focus() }`). A queue row is a plain `<button>`, so every click on a row was
such a click. The live run lost every key in the window after the first row click: the queue's
Enter did not play the selected row, arrow keys did nothing, and no other chord fired, while pointer
presses kept working. The window also loses the focus when the focused node is removed, which is
what closing an armed Search field or the queue filter does
(`clear_interaction_state_for_removed_node`).

## How the code upholds it today

- The root component paints `id: "mbv-root"` on `main` and takes the focus once on mount
  (`window/app.rs`).
- `MbvApplication::window_event` calls `restore_root_focus` after the shell has handled every
  window event, so the repair runs on the same event that cleared the focus and before any later
  keydown.
- `restore_root_focus` finds the root by its id and returns early while the focus already sits
  inside it (`focus_within` walks the focused node's ancestors). A text input the click landed on,
  a field a shortcut armed, and the root itself all keep their focus; only a focus outside the root
  — the `<html>` element, or none — is replaced.
- `set_focus_to` on a removed node id returns `false` and changes nothing, so a stale root id costs
  nothing.

## Where it still fails

- A keydown that arrives between the clearing event and the repair would still be dropped. The
  repair runs inside the same `window_event` call as the clear, so no other event can interleave.
- The repair depends on the root keeping its id. A root that drops `ROOT_ELEMENT_ID`, or a second
  element that takes it, leaves the repair with nothing to focus and the keyboard dead again; the
  headless test in `window/launch/tests.rs` covers the id and the repair, not the root's markup.
- `blitz-dom`'s clear is a pinned crate's behaviour. An upgrade that focuses the clicked element
  instead (as a browser focuses a `button`) needs a re-read of the repair: it only fills a focus
  outside the root, so a click that focused a row button would keep that button's focus and the
  window's `keydown` would still bubble to the root only while the row stays mounted.
