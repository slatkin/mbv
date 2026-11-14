//! Headless tests of the Settings button's press handler (D16): a disabled
//! button takes no activation — blitz-dom dispatches Click on disabled
//! targets, so the handler itself returns early — and an enabled one runs
//! `onpress`, the same command the keyboard's Enter runs (design D11's
//! pointer contract). A headless document cannot fire a DOM click, so the
//! tests call the named handler the `onclick` wiring calls.

use std::cell::Cell;
use std::rc::Rc;

use dioxus_native::prelude::*;

use super::press;

/// A press counter over the callback the handler runs.
fn counted() -> (Rc<Cell<usize>>, Callback<()>) {
    let calls = Rc::new(Cell::new(0));
    let counter = Rc::clone(&calls);
    let onpress = Callback::new(move |()| counter.set(counter.get() + 1));
    (calls, onpress)
}

/// Contract (design D11's pointer contract): an enabled button's press runs
/// `onpress`.
#[test]
fn an_enabled_button_press_runs_onpress() {
    let (calls, onpress) = counted();
    press(false, Some(onpress));
    assert_eq!(calls.get(), 1);
}

/// Contract (design D11's pointer contract): a disabled button's press takes
/// no activation.
#[test]
fn a_disabled_button_press_takes_no_activation() {
    let (calls, onpress) = counted();
    press(true, Some(onpress));
    assert_eq!(calls.get(), 0);
}
