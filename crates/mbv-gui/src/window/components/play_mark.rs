//! The shared play look (gui-played-row-marks design D1): the one derivation
//! and the one mark painter every row that carries a play state uses — the
//! flat row, the tree row, the Workspace table, and the queue. [`RowPlay`]
//! maps a row's carried state to `Unplayed` or `Played`, and its look methods
//! hand the painters the title tone and weight, the secondary tone, the
//! trailing tone, and the thumb opacity, so no painter restates a value.
//! [`PlayMark`] paints the mark itself: every row of a list that carries a
//! play state keeps the same width for it, so no row moves when its state
//! changes. [`PlayCluster`] paints the one trailing cluster — the mark, then
//! the trailing content — that carries a row's hover swap (design D3), so the
//! cluster's mechanics have one owner.
//!
//! A row that paints its own progress mark carries no play mark (user request
//! 2026-11-26): the thumb's resume run on a flat Library row, the resume run on
//! a Workspace row, and the progress ring on a queue row each already say the
//! item is in progress. [`RowPlay::mark_of_row`] and [`RowPlay::mark_of_queue`]
//! own that rule, so no painter restates it. A nested tree row paints no
//! progress mark, so it keeps its play mark.
//!
//! A sibling module of `list_row` and not a part of it, because the queue and
//! the Workspace table use the same rule and `list_row` is the Library and
//! Playlists row vocabulary.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiHomeRowStateFrame;
use crate::bridge::frame::queue::GuiSemanticState;
use crate::window::icons::Check;
use crate::window::library::episode::{episode_progress, is_played_episode};
use crate::window::theme::{EfToken, ef_var};

/// The mark slot's width and height in logical pixels (design D1): the larger
/// of the two marks, so the trailing text never moves when the state changes.
pub const PLAY_MARK_SLOT_PX: f32 = 12.0;

/// The unplayed dot's width and height in logical pixels (design D1).
pub const PLAY_DOT_PX: f32 = 8.0;

/// The played check's size in CSS pixels (design D1).
pub const PLAY_CHECK_PX: u16 = 12;

/// The gap between the mark and the trailing text in the trailing cluster, in
/// logical pixels (design D3).
pub const PLAY_MARK_GAP_PX: f32 = 6.0;

/// An unplayed row's title weight (design D1).
pub const UNPLAYED_TITLE_WEIGHT: f32 = 600.0;

/// A played row's title weight (design D1).
pub const PLAYED_TITLE_WEIGHT: f32 = 400.0;

/// A played row's thumb opacity (design D1, D2): a layer alpha, which the
/// renderer paints — no `filter`, which it drops.
pub const PLAYED_THUMB_OPACITY: f32 = 0.35;

/// The one played-state derivation (design D1): a row is unplayed or played,
/// and its carried state decides which. Every painter that carries a play
/// state derives through this type and never restates the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowPlay {
    /// The row is not played through.
    Unplayed,
    /// The row is played through.
    Played,
}

impl RowPlay {
    /// The derivation a Library row's carried state maps to (design D1): the
    /// played test stays `is_played_episode`, so the rule is still one
    /// function.
    #[must_use]
    pub fn of_row(state: &GuiHomeRowStateFrame) -> Self {
        if is_played_episode(state) {
            Self::Played
        } else {
            Self::Unplayed
        }
    }

    /// The mark a Library row paints (user request 2026-11-26): none while the
    /// row paints its own resume run, which carries the fact, and the state's
    /// own mark otherwise. The row's look still derives from [`Self::of_row`],
    /// so such a row keeps the unplayed look. A nested tree row paints no
    /// resume run, so it keeps the mark [`Self::of_row`] gives it.
    #[must_use]
    pub fn mark_of_row(state: &GuiHomeRowStateFrame) -> Option<Self> {
        match episode_progress(state) {
            Some(_) => None,
            None => Some(Self::of_row(state)),
        }
    }

    /// The mark a queue row paints (user request 2026-11-26): none while the
    /// row shows its progress ring, because the ring carries the in-progress
    /// fact. The row's look still derives from [`Self::of_queue`], so an
    /// in-progress row keeps the unplayed look.
    #[must_use]
    pub fn mark_of_queue(state: &GuiSemanticState) -> Option<Self> {
        match state {
            GuiSemanticState::Active { progress: Some(_) } => None,
            _ => Self::of_queue(state),
        }
    }

    /// The derivation a queue row's carried state maps to (design D1):
    /// `None` on the now-playing row, whose green look carries no mark.
    #[must_use]
    pub fn of_queue(state: &GuiSemanticState) -> Option<Self> {
        match state {
            GuiSemanticState::Played => Some(Self::Played),
            GuiSemanticState::Ordinary | GuiSemanticState::Active { .. } => Some(Self::Unplayed),
            GuiSemanticState::NowPlaying { .. } => None,
        }
    }

    /// The title's tone (design D1).
    #[must_use]
    pub const fn title_tone(self) -> &'static str {
        match self {
            Self::Unplayed => ef_var(EfToken::Fg),
            Self::Played => ef_var(EfToken::Grey1),
        }
    }

    /// The title's weight (design D1).
    #[must_use]
    pub const fn title_weight(self) -> f32 {
        match self {
            Self::Unplayed => UNPLAYED_TITLE_WEIGHT,
            Self::Played => PLAYED_TITLE_WEIGHT,
        }
    }

    /// The subtitle's tone (design D1).
    #[must_use]
    pub const fn secondary_tone(self) -> &'static str {
        match self {
            Self::Unplayed => ef_var(EfToken::Grey2),
            Self::Played => ef_var(EfToken::Grey1),
        }
    }

    /// The trailing text's tone (design D1): `None` on an unplayed row, which
    /// keeps the painter's own tone.
    #[must_use]
    pub const fn trail_tone(self) -> Option<&'static str> {
        match self {
            Self::Unplayed => None,
            Self::Played => Some(ef_var(EfToken::Grey1)),
        }
    }

    /// The thumb's opacity (design D1, D2): `None` on an unplayed row, which
    /// keeps full strength.
    #[must_use]
    pub const fn thumb_opacity(self) -> Option<f32> {
        match self {
            Self::Unplayed => None,
            Self::Played => Some(PLAYED_THUMB_OPACITY),
        }
    }

    /// The `data-play-mark` value the slot carries (design D1).
    #[must_use]
    pub const fn mark_value(self) -> &'static str {
        match self {
            Self::Unplayed => "unplayed",
            Self::Played => "played",
        }
    }

    /// The slot's accessible label while the mark is meaningful (design D1).
    #[must_use]
    pub const fn mark_label(self) -> &'static str {
        match self {
            Self::Unplayed => "Unplayed",
            Self::Played => "Played",
        }
    }
}

/// The props of the trailing cluster.
#[derive(Props, Clone, PartialEq)]
pub struct PlayClusterProps {
    /// The trail class the cluster carries (design D3): the `row-hover-trail`
    /// marker while the row's hover controls cover the trailing end, so the
    /// mark and the trailing content hide together.
    pub class: &'static str,
    /// The row's derived play state: `None` for the empty slot.
    pub play: Option<RowPlay>,
    /// The trailing content that shares the cluster with the mark.
    pub children: Element,
}

impl std::fmt::Debug for PlayClusterProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlayClusterProps")
            .field("class", &self.class)
            .field("play", &self.play)
            .finish_non_exhaustive()
    }
}

/// The trailing cluster (design D3): the one span — the play mark, then the
/// trailing content at the shared gap — that carries the row's hover swap, so
/// the mark and the content hide together while the hover controls show. The
/// flat row, the tree row, and the Workspace table all paint their trailing
/// end through this component, so the cluster's mechanics have one owner
/// (invariant 38).
#[component]
pub fn PlayCluster(props: PlayClusterProps) -> Element {
    let class = props.class;
    rsx! {
        span {
            class: "{class} flex items-center",
            style: "gap: {PLAY_MARK_GAP_PX}px;",
            PlayMark { play: props.play }
            {props.children}
        }
    }
}

/// The props of the play mark.
#[derive(Props, Debug, Clone, Copy, PartialEq)]
pub struct PlayMarkProps {
    /// The row's derived play state: `None` on the now-playing queue row and
    /// the playing Workspace row, which paint the empty slot so their
    /// trailing text stays aligned with the other rows.
    pub play: Option<RowPlay>,
}

/// The play mark (design D1): one `flex-none` slot span, `PLAY_MARK_SLOT_PX`
/// square, that centres its content and names the state in `data-play-mark`.
/// An unplayed row paints an 8 px circle; a played row the Lucide check; no
/// state the empty slot with no role and no label.
#[component]
pub fn PlayMark(props: PlayMarkProps) -> Element {
    let value = props.play.map_or("none", RowPlay::mark_value);
    let label = props.play.map(RowPlay::mark_label);
    let slot_style = format!("width: {PLAY_MARK_SLOT_PX}px; height: {PLAY_MARK_SLOT_PX}px;");
    let dot_style = format!(
        "width: {PLAY_DOT_PX}px; height: {PLAY_DOT_PX}px; border-radius: 50%; background: {};",
        ef_var(EfToken::Green)
    );
    rsx! {
        span {
            class: "flex flex-none items-center justify-center",
            style: "{slot_style}",
            "data-play-mark": value,
            "role": label.map(|_| "img"),
            "aria-label": label,
            match props.play {
                Some(RowPlay::Unplayed) => rsx! {
                    span { style: "{dot_style}" }
                },
                Some(RowPlay::Played) => rsx! {
                    Check {
                        size: PLAY_CHECK_PX,
                        color: ef_var(EfToken::Grey1).to_owned(),
                        aria_hidden: true,
                    }
                },
                None => rsx! {},
            }
        }
    }
}

#[cfg(test)]
mod tests;
