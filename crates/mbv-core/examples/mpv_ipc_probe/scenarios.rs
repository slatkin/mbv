//! Design D4 scenarios. All media is synthetic `av://lavfi:` input.

const AUDIO: &str = "av://lavfi:sine=frequency=440:duration=1";
const AUDIO_LONG: &str = "av://lavfi:sine=frequency=440:duration=5";
const VIDEO: &str = "av://lavfi:testsrc=duration=1:size=64x64";
/// Replaced with a path under the scratch directory that does not exist.
pub(crate) const MISSING: &str = "@missing";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Prop {
    Flag(bool),
    Int(i64),
}

#[derive(Debug)]
pub(crate) enum Step {
    Cmd(&'static [&'static str]),
    Set(&'static str, Prop),
    /// Collect events until this many events with this name arrive.
    Wait(&'static str, usize),
    /// Send `quit` and record until the event stream ends.
    Quit,
}

#[derive(Debug)]
pub(crate) struct Scenario {
    pub(crate) id: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) options: &'static [(&'static str, &'static str)],
    pub(crate) steps: &'static [Step],
}

pub(crate) const SCENARIOS: [Scenario; 10] = [
    Scenario {
        id: "S1",
        summary: "two-item audio playlist plays to natural EOF (gapless-audio=weak)",
        options: &[("gapless-audio", "weak")],
        steps: &[
            Step::Cmd(&["loadfile", AUDIO]),
            Step::Cmd(&["loadfile", AUDIO, "append"]),
            Step::Wait("end-file", 2),
        ],
    },
    Scenario {
        id: "S2",
        summary: "two playlist-pos writes in quick succession (superseded jump)",
        options: &[],
        steps: &[
            Step::Cmd(&["loadfile", AUDIO_LONG]),
            Step::Cmd(&["loadfile", AUDIO_LONG, "append"]),
            Step::Cmd(&["loadfile", AUDIO_LONG, "append"]),
            Step::Wait("playback-restart", 1),
            Step::Set("playlist-pos", Prop::Int(1)),
            Step::Set("playlist-pos", Prop::Int(2)),
            Step::Wait("playback-restart", 1),
            Step::Cmd(&["stop"]),
            Step::Wait("end-file", 1),
        ],
    },
    Scenario {
        id: "S3",
        summary: "playlist-next from mpv's side",
        options: &[],
        steps: &[
            Step::Cmd(&["loadfile", AUDIO_LONG]),
            Step::Cmd(&["loadfile", AUDIO_LONG, "append"]),
            Step::Wait("playback-restart", 1),
            Step::Cmd(&["playlist-next"]),
            Step::Wait("playback-restart", 1),
            Step::Cmd(&["stop"]),
            Step::Wait("end-file", 1),
        ],
    },
    Scenario {
        id: "S4",
        summary: "load-then-play: append with pause=yes, then unpause",
        options: &[],
        steps: &[
            Step::Set("pause", Prop::Flag(true)),
            Step::Cmd(&["loadfile", AUDIO]),
            Step::Cmd(&["loadfile", AUDIO, "append"]),
            Step::Wait("file-loaded", 1),
            Step::Set("pause", Prop::Flag(false)),
            Step::Wait("end-file", 2),
        ],
    },
    Scenario {
        id: "S5",
        summary: "missing file between two good items",
        options: &[],
        steps: &[
            Step::Cmd(&["loadfile", AUDIO]),
            Step::Cmd(&["loadfile", MISSING, "append"]),
            Step::Cmd(&["loadfile", AUDIO, "append"]),
            Step::Wait("end-file", 3),
        ],
    },
    Scenario {
        id: "S6",
        summary: "audio output init failure (audio-device=alsa/mbv-probe-missing)",
        options: &[("ao", "alsa"), ("audio-device", "alsa/mbv-probe-missing")],
        steps: &[Step::Cmd(&["loadfile", AUDIO]), Step::Wait("end-file", 1)],
    },
    Scenario {
        id: "S7",
        summary: "script-message mbv-next-up-play sent by a command",
        options: &[],
        steps: &[
            Step::Cmd(&["loadfile", AUDIO_LONG]),
            Step::Wait("playback-restart", 1),
            Step::Cmd(&["script-message", "mbv-next-up-play"]),
            Step::Wait("client-message", 1),
            Step::Cmd(&["stop"]),
            Step::Wait("end-file", 1),
        ],
    },
    Scenario {
        id: "S8",
        summary: "quit sent while a load is pending",
        options: &[],
        steps: &[Step::Cmd(&["loadfile", AUDIO]), Step::Quit],
    },
    Scenario {
        id: "S9",
        summary: "absolute seek while paused, then unpause",
        options: &[],
        steps: &[
            Step::Set("pause", Prop::Flag(true)),
            Step::Cmd(&["loadfile", AUDIO_LONG]),
            Step::Wait("playback-restart", 1),
            Step::Cmd(&["seek", "3", "absolute"]),
            Step::Wait("playback-restart", 1),
            Step::Set("pause", Prop::Flag(false)),
            Step::Wait("end-file", 1),
        ],
    },
    Scenario {
        id: "S10",
        summary: "video item: video-params/h and current-tracks/video/image changes",
        options: &[],
        steps: &[Step::Cmd(&["loadfile", VIDEO]), Step::Wait("end-file", 1)],
    },
];
