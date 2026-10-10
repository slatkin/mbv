//! Home's input translation over the flat carrier. Stubbed until the input
//! port (`openspec/changes/home-flat-recent-groups` task 1.3).

use tuirealm::event::KeyEvent;

use super::HomeContent;
use crate::media_list::MediaListSurfaceInput;
use mbv_ui_msg::{LeafKeyResult, Msg};

impl HomeContent {
    pub(super) fn handle_key(&mut self, _key: &KeyEvent) -> Option<Msg> {
        None
    }

    pub(super) fn activate_selected_row(&mut self) -> Option<Msg> {
        None
    }

    pub(super) fn on_list_slot_event(&mut self, _input: MediaListSurfaceInput) -> Option<Msg> {
        None
    }

    pub(super) fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None => LeafKeyResult::Unhandled,
        }
    }
}
