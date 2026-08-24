//! Minimal Playback-owned attribute carrier for precedence gates (design
//! `key_policy` table, entries `confirm_skip_intro`/`confirm_next_up`).
//!
//! Of the six `CONTEXT_STACK` entries above `view_dispatch` that `key_policy.rs`
//! marked `KeyPolicyGate::Custom`, only these two actually depend on ephemeral
//! `App` state: `handle_key_confirm_skip_intro` bails out via `?` unless
//! `App.skip_intro_end_ticks.is_some()`, and `handle_key_confirm_next_up`
//! bails out unless `App.next_up_item.is_some()`. (`clear_queue_prompt_c` and
//! `visualizer` turned out to be unconditional key matches on inspection --
//! see their corrected `KeyPolicyGate::Always` gates in `key_policy.rs` --
//! and `playback` resolves per-key via `resolve_key`, which can't reduce to a
//! static attribute at all.) This component exists so a future
//! `SubClause::HasAttrValue` guard on either of the two real entries reads
//! genuine state instead of re-deriving it.
//!
//! Mounted at `ComponentId::Playback` -- the owner `KEY_POLICY` already
//! assigns both entries to. This is scaffolding, not the Playback surface
//! itself (task 4.10): it owns nothing but these two attributes and paints
//! nothing.
//!
//! TODO(migrate-tui-to-tuirealm): superseded by the real Playback component
//! at task 4.10, which must preserve or re-derive these same two attributes
//! for whatever entries still read them by that point.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, Props, QueryResult};
use tuirealm::state::State;

use super::msg::Msg;
use super::user_event::UserEvent;

/// Set when `App.skip_intro_end_ticks.is_some()` -- mirrors
/// `handle_key_confirm_skip_intro`'s own eligibility guard exactly.
pub const ATTR_SKIP_INTRO_PROMPT_VISIBLE: Attribute =
    Attribute::Custom("skip_intro_prompt_visible");
/// Set when `App.next_up_item.is_some()` -- mirrors
/// `handle_key_confirm_next_up`'s own eligibility guard exactly.
pub const ATTR_NEXT_UP_PROMPT_VISIBLE: Attribute = Attribute::Custom("next_up_prompt_visible");

/// Attribute-only placeholder mounted at `ComponentId::Playback`. Never made
/// active and never subscribed: `on` is unreachable in practice but required
/// by `AppComponent`.
pub struct PlaybackGatesComponent {
    props: Props,
}

impl PlaybackGatesComponent {
    pub fn new() -> Self {
        let mut props = Props::default();
        props.set(ATTR_SKIP_INTRO_PROMPT_VISIBLE, AttrValue::Flag(false));
        props.set(ATTR_NEXT_UP_PROMPT_VISIBLE, AttrValue::Flag(false));
        Self { props }
    }
}

impl Component for PlaybackGatesComponent {
    // Paints nothing: this component is never rendered.
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        self.props.get_for_query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for PlaybackGatesComponent {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::ComponentId;
    use tuirealm::application::Application;
    use tuirealm::listener::EventListenerCfg;

    #[test]
    fn defaults_to_not_visible() {
        let c = PlaybackGatesComponent::new();
        assert_eq!(
            c.query(ATTR_SKIP_INTRO_PROMPT_VISIBLE).unwrap(),
            AttrValue::Flag(false)
        );
        assert_eq!(
            c.query(ATTR_NEXT_UP_PROMPT_VISIBLE).unwrap(),
            AttrValue::Flag(false)
        );
    }

    /// Proves the attribute genuinely round-trips through the `Application`
    /// registry (not just the bare struct) -- this is what a future
    /// `SubClause::HasAttrValue(ComponentId::Playback, ATTR_*, ..)` guard
    /// will actually read.
    #[test]
    fn attr_round_trips_through_application() {
        let mut app: Application<ComponentId, Msg, UserEvent> =
            Application::init(EventListenerCfg::default());
        app.mount(
            ComponentId::Playback,
            Box::new(PlaybackGatesComponent::new()),
            vec![],
        )
        .expect("mount");

        app.attr(
            &ComponentId::Playback,
            ATTR_SKIP_INTRO_PROMPT_VISIBLE,
            AttrValue::Flag(true),
        )
        .expect("attr");

        let skip_intro = app
            .query(&ComponentId::Playback, ATTR_SKIP_INTRO_PROMPT_VISIBLE)
            .expect("query ok")
            .expect("some");
        assert_eq!(skip_intro, AttrValue::Flag(true));

        // The other gate is untouched by writing the first.
        let next_up = app
            .query(&ComponentId::Playback, ATTR_NEXT_UP_PROMPT_VISIBLE)
            .expect("query ok")
            .expect("some");
        assert_eq!(next_up, AttrValue::Flag(false));
    }
}
