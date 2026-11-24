//! The Feeds section's edit (the contract of `feeds.test.ts`, design D9): an
//! edit sends `feedEdit` with the subscription's URL as its identity — the
//! config list has no other — so the command addresses the subscription the
//! frame painted even if the list moved since.

use mbv_queue::FeedKind;

use crate::bridge::frame::settings::GuiSettingsContent;
use crate::bridge::frame::settings::feeds::GuiSettingsFeed;
use crate::window::settings::state::{SettingsForm, SettingsState};

use super::{Facts, run, settings_commands};

/// The Feeds section's content around the form.
fn feeds_content() -> GuiSettingsContent {
    GuiSettingsContent::Feeds {
        subscriptions: vec![GuiSettingsFeed {
            name: "Sub One".to_owned(),
            url: "https://sub.one/feed".to_owned(),
            kind: FeedKind::Audio,
        }],
        adding: false,
    }
}

/// Contract (`feeds.test.ts`): an edit sends feedEdit with the subscription
/// URL as identity.
#[test]
fn an_edit_sends_feed_edit_with_the_subscription_url_as_identity() {
    let (facts, rx) = run(|state: &SettingsState, _facts: &Facts| {
        state.open_feed_form(Some(&GuiSettingsFeed {
            name: "Sub One".to_owned(),
            url: "https://sub.one/feed".to_owned(),
            kind: FeedKind::Audio,
        }));
        state.update_feed_form(|form| {
            form.name = "Renamed".to_owned();
            form.kind = FeedKind::Video;
        });
        state.focus_settings_field(SettingsForm::Feed);
        state.submit_settings_form(&feeds_content());
    });
    assert_eq!(*facts, [] as [&str; 0]);
    assert_eq!(
        settings_commands(&rx),
        vec![crate::gui_core::GuiSettingsCommand::FeedEdit {
            url: "https://sub.one/feed".to_owned(),
            name: "Renamed".to_owned(),
            kind: FeedKind::Video,
        }],
    );
}
