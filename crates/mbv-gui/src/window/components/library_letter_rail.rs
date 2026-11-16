//! The Library list's letter rail (`replace-tauri-with-blitz` task 7.2, the
//! port of `LibraryLetterRail`; add-gui-library-panel design D7): A
//! to Z beside the list, one 22 px letter a row. A letter no painted row
//! carries is disabled, so it cannot scroll; a press scrolls to the first row
//! under the letter. The rail marks no letter: the lists that paint it keep no
//! cursor (`gui-pointer-target-interaction` D6). The canvas gives the rail no
//! key (design D7): its keyboard path is the `/` filter, and the rail's letters
//! are pointer targets only.

use dioxus_native::prelude::*;

use crate::window::library::list::RailLetter;
use crate::window::library::state::LibraryState;
use crate::window::theme::canvas_var;

/// The props of the letter rail (the `LibraryLetterRail` props, in
/// order).
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryLetterRailProps {
    /// The A–Z letters and the first row under each, from the painted rows
    /// (design D7).
    pub letters: Vec<RailLetter>,
    /// What a press on an enabled letter does: the Music list's jump by
    /// default, and the Shows list's own while it paints the rail (design
    /// D12).
    #[props(default)]
    pub jump: Option<Callback<char>>,
}

/// A letter's text colour: the muted text of an enabled letter, and the
/// canvas's dimmest for a disabled one.
fn letter_color(entry: &RailLetter) -> String {
    if entry.row_id.is_none() {
        return "#4f5b58".to_owned();
    }
    canvas_var("muted")
}

/// The letter rail.
#[component]
pub fn LibraryLetterRail(props: LibraryLetterRailProps) -> Element {
    let library = consume_context::<LibraryState>();

    rsx! {
        nav {
            class: "flex flex-none flex-col items-center",
            "aria-label": "Jump to letter",
            style: "width: 28px; padding-top: 4px;",
            "data-library-region": "rail",
            for entry in props.letters.iter() {
                {
                    let letter = entry.letter;
                    let disabled = entry.row_id.is_none();
                    let cursor = if disabled { "default" } else { "pointer" };
                    let color = letter_color(entry);
                    // A disabled letter paints `disabled`; an enabled one paints
                    // no mark, as the former client `undefined` values did not.
                    let mut marks = Vec::new();
                    if disabled {
                        marks.push(Attribute::new("disabled", "true", None, false));
                    }
                    let jump = props.jump;
                    let jump_library = library.clone();
                    rsx! {
                        button {
                            "type": "button",
                            class: "flex items-center justify-center font-mono",
                            style: "width: 24px; height: 22px; padding: 0; border: 0; border-radius: 3px; font-size: 11px; cursor: {cursor}; color: {color};",
                            "data-library-rail-letter": "{letter}",
                            onclick: move |_| match jump {
                                Some(jump) => jump.call(letter),
                                None => jump_library.jump_to_letter(letter),
                            },
                            ..marks,
                            "{letter}"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
