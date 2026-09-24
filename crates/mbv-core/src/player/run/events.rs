mod event_classifiers;
mod playlist_drift;
mod queue_advance;
mod restart;
mod termination;
mod track_progress;

#[cfg(test)]
pub(in crate::player) use event_classifiers::{
    is_clocked_audio_error, is_superseded_jump_end_file, provider_lifecycle_close_pos,
};
