//! Classifier and decoder contracts on mpv JSON IPC traces recorded by
//! `mpv_ipc_probe` (`openspec/changes/mpv-ipc-event-probe`, design D6).

use super::ipc::DecodeError;
use super::{EndFileReason, MpvEvent, MpvFailure};
use crate::run::{is_clocked_audio_error, is_superseded_jump_end_file};

/// Decodes every line of a fixture. A line that does not decode fails the test.
fn decode_fixture(fixture: &str) -> Vec<MpvEvent> {
    fixture
        .lines()
        .map(|line| MpvEvent::from_ipc_line(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn first_failure(events: &[MpvEvent]) -> &MpvFailure {
    events
        .iter()
        .find_map(|event| match event {
            MpvEvent::Failed(failure) => Some(failure),
            _ => None,
        })
        .unwrap_or_else(|| panic!("fixture has no failure event"))
}

// S2: a second jump while the first target is loading.
#[test]
fn s2_stop_end_file_is_superseded_jump_debris() {
    let events = decode_fixture(include_str!("fixtures/S2.jsonl"));
    assert!(events.iter().any(|event| matches!(
        event,
        MpvEvent::EndFile(reason) if is_superseded_jump_end_file(*reason, None, false)
    )));
}

// S6: audio output initialization fails.
#[test]
fn s6_audio_output_failure_is_clocked_audio_error() {
    let events = decode_fixture(include_str!("fixtures/S6.jsonl"));
    let failure = first_failure(&events);
    assert!(matches!(failure, MpvFailure::AudioOutputInit));
    assert!(is_clocked_audio_error(failure, true));
}

// S5: the file does not exist.
#[test]
fn s5_missing_file_is_not_clocked_audio_error() {
    let events = decode_fixture(include_str!("fixtures/S5.jsonl"));
    let failure = first_failure(&events);
    assert!(matches!(failure, MpvFailure::Other(_)));
    assert!(!is_clocked_audio_error(failure, true));
}

// S1: the file plays to its natural end.
#[test]
fn s1_natural_end_is_eof() {
    let events = decode_fixture(include_str!("fixtures/S1.jsonl"));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, MpvEvent::EndFile(EndFileReason::Eof)))
    );
}

#[test]
fn truncated_line_is_a_decode_error() {
    let error = MpvEvent::from_ipc_line(r#"{"event":"property-change","name":"pa"#);
    assert!(matches!(error, Err(DecodeError::NotJson(_))));
}
