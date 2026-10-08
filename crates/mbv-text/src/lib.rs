//! Text helpers for search matching and feed sanitizing.
//!
//! `fuzzy_match` holds the one acceptance rule over `SkimMatcherV2` scoring, `html`
//! decodes entities, and `text_safety` predicates control characters. Keybinding
//! parsing lives in `mbv-keybinds` and painting in `mbv-render`, not here.

pub mod fuzzy_match;
pub mod html;
pub mod text_safety;
