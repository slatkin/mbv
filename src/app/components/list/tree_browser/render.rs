//! TuiRealm component and retained-frame painting for the tree owner.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::TreeBrowser;

impl<Target: Clone + Eq + std::hash::Hash> Component for TreeBrowser<Target> {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let (claim_rect, content_rect) = self.configured_geometry.unwrap_or((area, area));
        self.reconcile_selection();
        self.paint.begin();
        let rows = self.visible_rows();
        crate::app::render::render_tree_browser(
            frame,
            claim_rect,
            &rows,
            self.focused,
            &mut self.marquee_text,
            &mut self.marquee_started_at,
        );
        let selected_row = rows
            .iter()
            .position(|row| row.selected)
            .and_then(|index| claim_rect.y.checked_add(index as u16))
            .map(|y| Rect::new(claim_rect.x, y, claim_rect.width, 1));
        self.paint.store_completed(
            claim_rect,
            content_rect,
            self.viewport_offset,
            self.retained_rows(claim_rect),
            selected_row,
        );
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}
