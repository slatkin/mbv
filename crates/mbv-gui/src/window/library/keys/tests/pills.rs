//! The `pills` contracts of `keys.test.ts` (D16), over the shared harness: the
//! pill rows hold no keyboard focus, so Space stays the core's key.

use super::*;

// Space with the pointer on a pill is sent to the core: the pill rows hold no
// keyboard focus (design D2 of `gui-pointer-target-interaction`, step 4).
#[test]
fn space_with_the_pointer_on_a_pill_is_sent_to_the_core() {
    let list = music_list(Vec::new());
    let (facts, _rx) = run(move |state, facts| {
        carry(state, list.clone(), music_detail(None));
        route(state, facts, "space", " ", false);
        facts.lock().expect("facts").push(ui_facts(state));
    });
    assert_eq!(facts[0], "space=Core(None)");
    assert!(facts[1].contains("pill_select=None"));
}
