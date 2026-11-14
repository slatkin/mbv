//! Headless tests of the pick button's press-inside handler (D16): a press
//! on the button or its open menu hides the content cursor — the Svelte
//! window's `hideSettingsCursor` ran on every pointerdown, presses on the
//! pick controls included — before the handler stops the press from
//! reaching the Settings screen root's press-outside close (task 8.3). A
//! headless document cannot fire a DOM pointer event, so the tests call the
//! named handler the `onpointerdown` wiring calls.

use std::sync::{Arc, Mutex, mpsc};

use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::press_inside;
use crate::core_thread::CoreInput;
use crate::window::channel::CoreLink;
use crate::window::settings::state::SettingsState;

/// The facts one scenario recorded, in its run order.
type Facts = Arc<Mutex<Vec<String>>>;

/// One scenario the harness runs against the built store.
type Scenario = std::rc::Rc<dyn Fn(&SettingsState, &Facts)>;

/// The root: creates the store once and runs the scenario in one effect.
fn harness((tx, facts, scenario): (mpsc::Sender<CoreInput>, Facts, Scenario)) -> Element {
    let state = use_hook(|| SettingsState::new(CoreLink::new(tx)));
    use_effect(move || {
        scenario(&state, &facts);
    });
    rsx! {
        div { "pick-harness" }
    }
}

/// Run one scenario against a fresh store and return the facts it recorded.
fn run(scenario: impl Fn(&SettingsState, &Facts) + 'static) -> Vec<String> {
    let (tx, _rx) = mpsc::channel::<CoreInput>();
    let facts: Facts = Arc::new(Mutex::new(Vec::new()));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            harness,
            (
                tx,
                Arc::clone(&facts),
                std::rc::Rc::new(move |state: &SettingsState, recorded: &Facts| {
                    scenario(state, recorded);
                }),
            ),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    for _ in 0..8 {
        if !doc.poll(None) {
            break;
        }
    }
    facts.lock().expect("facts").clone()
}

/// Contract (design D11's pointer contract): a press inside the pick
/// controls hides the content cursor, as every Svelte pointerdown did.
#[test]
fn a_press_inside_the_pick_controls_hides_the_cursor() {
    let recorded = run(|state: &SettingsState, facts: &Facts| {
        state.show_settings_cursor();
        facts
            .lock()
            .expect("facts")
            .push(state.ui.cloned().keyboard.to_string());
        press_inside(state);
        facts
            .lock()
            .expect("facts")
            .push(state.ui.cloned().keyboard.to_string());
    });
    assert_eq!(recorded, vec!["true", "false"]);
}
