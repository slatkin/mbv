//! Shell handling of the Twitch URL prompt (twitch-url-playback task 5.5,
//! design D7): the opener mounts it on the focus stack; a refused URL keeps
//! it mounted with the refusal shown and sends nothing; an accepted URL
//! unmounts it and submits the play-now Twitch item; an owner without the
//! `twitch-queue` capability receives nothing and keeps the prompt open.

use super::*;
use crate::app::dispatch::action::{Command, ShellCommand};
use mbv_components::TwitchUrlComponent;
use mbv_queue::QueueItem;
use mbv_ui_msg::{ComponentId, ModalId, Msg, ShellRequest, TwitchUrlIntent};

fn prompt_id() -> ComponentId {
    ComponentId::Modal(ModalId::TwitchUrl)
}

/// An owner compat whose `twitch-queue` capability is `supported` (the stub
/// installs it directly, standing in for the Hello handshake).
fn compat_with_twitch(supported: bool) -> mbv_ctrl::CtrlCompatibility {
    let mut compat = mbv_ctrl::CtrlCompatibility::current();
    compat.supports_twitch_queue = supported;
    compat
}

fn model_with_owner(supported: bool) -> (Model, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx_and_compat(
        Vec::new(),
        compat_with_twitch(supported),
    );
    (Model::new(app), cmd_rx)
}

fn mount_prompt(model: &mut Model) {
    model.dispatch_router_command(&Command::Shell(ShellCommand::OpenTwitchUrl));
}

fn submit(model: &mut Model, url: &str) {
    model.handle_terminal_message(
        Msg::Shell(Box::new(ShellRequest::TwitchUrlIntent(
            TwitchUrlIntent::Submit(url.to_string()),
        ))),
        &mut false,
        &mut false,
    );
}

fn queue_commands(cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) -> Vec<mbv_ctrl::CtrlCmd> {
    cmd_rx
        .try_iter()
        .filter(|command| {
            matches!(
                command,
                mbv_ctrl::CtrlCmd::UnifiedQueueReplace { .. }
                    | mbv_ctrl::CtrlCmd::UnifiedQueueAppend { .. }
            )
        })
        .collect()
}

/// The replaces the channel holds, as `(slots, start_idx)` pairs.
fn replaces(
    cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> Vec<(Vec<mbv_ctrl::UnifiedQueueSlot>, Option<usize>)> {
    cmd_rx
        .try_iter()
        .filter_map(|command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
                slots, start_idx, ..
            } => Some((slots, start_idx)),
            _ => None,
        })
        .collect()
}

/// A non-Twitch URL is refused in the prompt: it stays mounted with the
/// refusal naming the host, and no queue command reaches the owner (spec
/// *The prompt accepts only Twitch hosts*, other-host scenario).
#[test]
fn refused_url_keeps_the_prompt_mounted_and_the_queue_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut model, cmd_rx) = model_with_owner(true);
    mount_prompt(&mut model);

    submit(&mut model, "https://www.youtube.com/watch?v=abc");

    assert!(model.application.mounted(&prompt_id()));
    let refusal = model
        .application
        .get_component(&prompt_id())
        .and_then(|component| component.as_any().downcast_ref::<TwitchUrlComponent>())
        .and_then(|prompt| prompt.refusal())
        .map(str::to_owned);
    assert_eq!(
        refusal.as_deref(),
        Some("not a Twitch URL: www.youtube.com is not a twitch.tv host")
    );
    assert!(
        queue_commands(&cmd_rx).is_empty(),
        "a refused URL must not reach the owner queue"
    );
}

/// An accepted URL closes the prompt and submits one play-now Twitch item
/// through the same replace op as other items (spec *Submit plays the URL
/// at once as a Twitch item*).
#[test]
fn accepted_url_submits_a_play_now_twitch_item_and_closes_the_prompt() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut model, cmd_rx) = model_with_owner(true);
    mount_prompt(&mut model);

    submit(&mut model, "https://www.twitch.tv/somechannel");

    assert!(
        !model.application.mounted(&prompt_id()),
        "an accepted URL must close the prompt"
    );
    let replaces = replaces(&cmd_rx);
    assert_eq!(replaces.len(), 1, "one replace op carries the item");
    let (slots, start_idx) = &replaces[0];
    assert_eq!(*start_idx, Some(0), "play-now starts the submitted item");
    assert_eq!(
        slots
            .iter()
            .map(|slot| match &slot.item {
                QueueItem::Twitch(item) => (slot.slot_id, item.url.as_str(), item.title.as_str()),
                other => panic!("unexpected queue item {other:?}"),
            })
            .collect::<Vec<_>>(),
        vec![(1, "https://www.twitch.tv/somechannel", "somechannel")],
        "the accepted URL replaces the owner queue with one Twitch item"
    );
}

/// An owner that did not advertise `twitch-queue` receives nothing
/// (capability first, design D6): the prompt stays mounted with the
/// owner-refusal text and no queue command is sent (ctrl-protocol spec,
/// *Client attached to an Owner without the capability*).
#[test]
fn owner_without_the_capability_sends_nothing() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut model, cmd_rx) = model_with_owner(false);
    mount_prompt(&mut model);

    submit(&mut model, "https://www.twitch.tv/somechannel");

    assert!(model.application.mounted(&prompt_id()));
    let refusal = model
        .application
        .get_component(&prompt_id())
        .and_then(|component| component.as_any().downcast_ref::<TwitchUrlComponent>())
        .and_then(|prompt| prompt.refusal())
        .map(str::to_owned);
    assert_eq!(
        refusal.as_deref(),
        Some("This Player owner cannot play Twitch items.")
    );
    assert!(
        queue_commands(&cmd_rx).is_empty(),
        "an incapable owner must receive no queue command"
    );
}
