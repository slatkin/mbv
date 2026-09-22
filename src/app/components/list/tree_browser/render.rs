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
        self.last_painted = Some(area);
        let (claim_rect, content_rect) = self.configured_geometry.unwrap_or((area, area));
        self.reconcile_selection();
        self.paint.begin();
        let visible_ids = self.visible_node_ids();
        let rows = self.visible_rows(&visible_ids);
        crate::app::render::render_tree_browser(
            frame,
            claim_rect,
            content_rect,
            &rows,
            visible_ids.len(),
            self.viewport_offset,
            self.focused,
            &mut self.marquee_text,
            &mut self.marquee_started_at,
        );
        let selected_row = rows
            .iter()
            .position(|row| row.selected)
            .and_then(|index| content_rect.y.checked_add(index as u16))
            .map(|y| Rect::new(content_rect.x, y, content_rect.width, 1));
        self.paint.store_completed(
            claim_rect,
            content_rect,
            self.viewport_offset,
            self.retained_rows(&visible_ids, claim_rect, content_rect),
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
