//! Translate a matched router binding into the semantic command it owns.
//! The transport split binds each payload-carrying variant to its
//! fixed call-site value here — the single site the old
//! `playback_command_for_key` table bound them — so the eligibility gate and
//! registry chord matching in `super::resolve_policy` fully decide whether a
//! command dispatches.

use super::KeyChord;
use super::KeyPolicyBinding;
use crate::app::dispatch::action::{
    AppCommand, Command, NavigationIntent, PanelIntent, PlaybackIntent, ShellCommand, VisualIntent,
};
use mbv_ui_model::playback::VOLUME_STEP;
use mbv_ui_model::settings::PanelFocus;
use mbv_ui_msg::{Key, TrackChipKind};

pub(in crate::app) fn command_for_policy(
    binding: KeyPolicyBinding,
    key: KeyChord,
) -> Option<Command> {
    command_for_simple_binding(binding)
        .or_else(|| command_for_navigation_binding(binding, key))
        .or_else(|| command_for_transport_binding(binding))
}

fn command_for_simple_binding(binding: KeyPolicyBinding) -> Option<Command> {
    match binding {
        KeyPolicyBinding::SettingsOpen => Some(NavigationIntent::ToggleSettings.into()),
        KeyPolicyBinding::SessionsOpen => Some(NavigationIntent::OpenSessions.into()),
        KeyPolicyBinding::PlaylistsOpen => Some(NavigationIntent::OpenPlaylists.into()),
        KeyPolicyBinding::SearchOpen => Some(NavigationIntent::OpenSearch.into()),
        KeyPolicyBinding::TwitchUrlOpen => Some(ShellCommand::OpenTwitchUrl.into()),
        KeyPolicyBinding::HelpOpen => Some(ShellCommand::OpenHelp.into()),
        KeyPolicyBinding::Quit => Some(AppCommand::Quit.into()),
        KeyPolicyBinding::PanelRight => Some(PanelIntent::FocusPanel(PanelFocus::Library).into()),
        KeyPolicyBinding::PanelLeft => Some(PanelIntent::FocusPanel(PanelFocus::Queue).into()),
        KeyPolicyBinding::PanelModeCycle => Some(PanelIntent::CyclePanelMode.into()),
        KeyPolicyBinding::CtrlL => Some(NavigationIntent::ForceClear.into()),
        KeyPolicyBinding::ClearQueue => Some(NavigationIntent::RequestClearQueue.into()),
        KeyPolicyBinding::F5 => Some(ShellCommand::RefreshCurrentView.into()),
        KeyPolicyBinding::Visualizer => Some(VisualIntent::ToggleVisualizer.into()),
        KeyPolicyBinding::HideVisualSlot => Some(VisualIntent::ToggleVisualSlotHidden.into()),
        // The GUI-only key context's commands (add-tauri-gui-client row 6.2):
        // the canvas's panel and queue-header actions, applied by the App's
        // panel dispatcher. The TUI Client never resolves them (its gate is
        // closed), so the arms are GUI-reachable only.
        KeyPolicyBinding::GuiCollapsePanel => Some(PanelIntent::TogglePanelCollapse.into()),
        KeyPolicyBinding::GuiFoldQueue => Some(PanelIntent::ToggleQueueFold.into()),
        KeyPolicyBinding::GuiCycleSubtitle => Some(PlaybackIntent::CycleOrToggleSubtitle.into()),
        KeyPolicyBinding::GuiRightColumnToggle => Some(PanelIntent::ToggleQueueRightColumn.into()),
        KeyPolicyBinding::GuiQueueFilterOpen => Some(PanelIntent::ActivateQueueFilter.into()),
        KeyPolicyBinding::GuiOpenTargetChooser => Some(PanelIntent::ToggleTargetChooser.into()),
        _ => None,
    }
}

fn command_for_navigation_binding(binding: KeyPolicyBinding, key: KeyChord) -> Option<Command> {
    match binding {
        KeyPolicyBinding::NextLibraryTab | KeyPolicyBinding::AltNextLibraryTab => {
            Some(NavigationIntent::NextLibraryTab.into())
        }
        KeyPolicyBinding::PreviousLibraryTab | KeyPolicyBinding::AltPreviousLibraryTab => {
            Some(NavigationIntent::PreviousLibraryTab.into())
        }
        KeyPolicyBinding::LibraryTabJump => match key.code {
            Key::Char(c @ '1'..='9') => {
                Some(NavigationIntent::SetLibraryTab((c as usize) - '1' as usize).into())
            }
            _ => None,
        },
        _ => None,
    }
}

fn command_for_transport_binding(binding: KeyPolicyBinding) -> Option<Command> {
    // The split transport set: each action binds the fixed payload its key's
    // call site carried before the bucket split.
    match binding {
        KeyPolicyBinding::TogglePlayPause => Some(PlaybackIntent::TogglePlayPause.into()),
        KeyPolicyBinding::Stop => Some(PlaybackIntent::Stop.into()),
        KeyPolicyBinding::SeekBack => Some(PlaybackIntent::SeekRelative(-5.0).into()),
        KeyPolicyBinding::SeekForward => Some(PlaybackIntent::SeekRelative(5.0).into()),
        // The canvas transport's deeper steps (add-tauri-gui-client row
        // 6.4, the "back 10 s, forward 30 s" spec): GUI-only actions whose
        // `seekBack`/`seekForward` regions press the same commands.
        KeyPolicyBinding::SeekBack10 => Some(PlaybackIntent::SeekRelative(-10.0).into()),
        KeyPolicyBinding::SeekForward30 => Some(PlaybackIntent::SeekRelative(30.0).into()),
        // The track chips' popover openers (the "every action is
        // reachable from the keyboard" spec): GUI-only actions whose
        // chords dispatch the same toggle the chips' `OpenTrackPopover`
        // press routes.
        KeyPolicyBinding::OpenAudioPopover => {
            Some(PlaybackIntent::ToggleTrackPopover(TrackChipKind::Audio).into())
        }
        KeyPolicyBinding::OpenSubtitlePopover => {
            Some(PlaybackIntent::ToggleTrackPopover(TrackChipKind::Subtitles).into())
        }
        KeyPolicyBinding::NextTrack => Some(PlaybackIntent::NextTrack.into()),
        KeyPolicyBinding::PreviousTrack => Some(PlaybackIntent::PreviousTrack.into()),
        KeyPolicyBinding::VolumeDown => Some(PlaybackIntent::AdjustVolume(-VOLUME_STEP).into()),
        KeyPolicyBinding::VolumeUp => Some(PlaybackIntent::AdjustVolume(VOLUME_STEP).into()),
        KeyPolicyBinding::ToggleMute => Some(PlaybackIntent::ToggleMute.into()),
        KeyPolicyBinding::CycleAudio => Some(PlaybackIntent::CycleAudio.into()),
        KeyPolicyBinding::CycleSubtitle => Some(PlaybackIntent::CycleOrToggleSubtitle.into()),
        KeyPolicyBinding::OpenIdleFeedLink => Some(VisualIntent::OpenIdleFeedLink.into()),
        _ => None,
    }
}
