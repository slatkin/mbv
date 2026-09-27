use libmpv2::Mpv;
use mbv_ctrl::player::{PlayerStatus, SubtitleChoice, SubtitlePrefs};
use std::sync::{Arc, Mutex};

const LANGS: &[(&[&str], &str)] = &[
    (&["en", "eng"], "English"),
    (&["fr", "fre", "fra"], "French"),
    (&["de", "ger", "deu"], "German"),
    (&["es", "spa"], "Spanish"),
    (&["it", "ita"], "Italian"),
    (&["pt", "por"], "Portuguese"),
    (&["ja", "jpn"], "Japanese"),
    (&["ko", "kor"], "Korean"),
    (&["zh", "chi", "zho"], "Chinese"),
    (&["ru", "rus"], "Russian"),
    (&["ar", "ara"], "Arabic"),
    (&["nl", "nld", "dut"], "Dutch"),
    (&["sv", "swe"], "Swedish"),
    (&["no", "nor"], "Norwegian"),
    (&["da", "dan"], "Danish"),
    (&["fi", "fin"], "Finnish"),
    (&["pl", "pol"], "Polish"),
    (&["cs", "cze", "ces"], "Czech"),
    (&["tr", "tur"], "Turkish"),
];

pub(in crate::player) fn lang_code_to_name(code: &str) -> &'static str {
    let code = code.to_lowercase();
    LANGS
        .iter()
        .find_map(|(codes, name)| codes.contains(&code.as_str()).then_some(*name))
        .unwrap_or("")
}

fn fmt_channels(n: i64) -> &'static str {
    match n {
        1 => "Mono",
        2 => "Stereo",
        6 => "5.1",
        8 => "7.1",
        _ => "",
    }
}

fn is_image_sub(codec: &str) -> bool {
    matches!(
        codec,
        "hdmv_pgs_subtitle" | "pgssub" | "dvd_subtitle" | "dvdsub" | "dvb_subtitle" | "xsub"
    )
}

/// Returns true if `label` begins with or contains the full language name `lang_pref`
/// (case-insensitive). Used to match audio/subtitle track labels against a preferred language.
fn label_matches_lang(label: &str, lang_pref: &str) -> bool {
    if lang_pref.is_empty() {
        return false;
    }
    let l = label.to_lowercase();
    let p = lang_pref.to_lowercase();
    l.starts_with(&p)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TrackInfo {
    pub(super) kind: String,
    pub(super) id: i64,
    pub(super) lang: String,
    pub(super) title: String,
    pub(super) codec: String,
    pub(super) selected: bool,
    pub(super) channels: i64,
    pub(super) forced: bool,
    pub(super) stream_index: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ParsedTracks {
    pub(super) audio_tracks: Vec<(i64, String)>,
    pub(super) sub_tracks: Vec<(i64, String, bool)>,
    pub(super) sub_track_stream_indexes: Vec<(i64, i64)>,
    pub(super) audio_id: i64,
    pub(super) audio_lang: String,
    pub(super) sub_id: i64,
    pub(super) sub_lang: String,
}

pub(super) fn parse_tracks(tracks: &[TrackInfo]) -> ParsedTracks {
    let mut parsed = ParsedTracks::default();
    for (index, track) in tracks.iter().enumerate() {
        let fallback = index + 1;
        match track.kind.as_str() {
            "audio" => {
                if track.selected {
                    parsed.audio_id = track.id;
                    parsed.audio_lang.clone_from(&track.lang);
                }
                let language = lang_code_to_name(&track.lang);
                let label = if !language.is_empty() {
                    let mut parts = vec![language.to_string(), track.codec.to_uppercase()];
                    let channels = fmt_channels(track.channels);
                    parts.extend((!channels.is_empty()).then(|| channels.to_string()));
                    parts.join(" ")
                } else if !track.title.is_empty() {
                    track.title.clone()
                } else if !track.lang.is_empty() {
                    track.lang.to_uppercase()
                } else {
                    format!("#{fallback}")
                };
                parsed.audio_tracks.push((track.id, label));
            }
            "sub" if !is_image_sub(&track.codec) => {
                if track.selected {
                    parsed.sub_id = track.id;
                    parsed.sub_lang.clone_from(&track.lang);
                }
                let language = lang_code_to_name(&track.lang);
                let base = if !track.title.is_empty() {
                    track.title.clone()
                } else if !language.is_empty() {
                    language.to_string()
                } else if !track.lang.is_empty() {
                    track.lang.to_uppercase()
                } else {
                    format!("#{fallback}")
                };
                let label = if track.forced {
                    format!("{base} (Forced)")
                } else {
                    base
                };
                parsed.sub_tracks.push((track.id, label, track.forced));
                if track.stream_index >= 0 {
                    parsed
                        .sub_track_stream_indexes
                        .push((track.id, track.stream_index));
                }
            }
            _ => {}
        }
    }
    parsed
}

/// Returns the audio id to select (`None` leaves it unchanged) and the
/// subtitle choice to apply.
pub(super) fn select_tracks(
    audio_tracks: &[(i64, String)],
    sub_tracks: &[(i64, String, bool)],
    audio_id: i64,
    audio_lang: &str,
    prefs: &SubtitlePrefs,
) -> (Option<i64>, SubtitleChoice) {
    (
        preferred_audio_track(audio_tracks, audio_id, &prefs.audio_lang),
        preferred_subtitle_track(sub_tracks, audio_lang, prefs),
    )
}

fn preferred_audio_track(tracks: &[(i64, String)], current_id: i64, language: &str) -> Option<i64> {
    if language.is_empty()
        || tracks
            .iter()
            .find(|(id, _)| *id == current_id)
            .is_some_and(|(_, label)| label_matches_lang(label, language))
    {
        None
    } else {
        tracks
            .iter()
            .find(|(_, label)| label_matches_lang(label, language))
            .map(|(id, _)| *id)
    }
}

fn preferred_subtitle_track(
    tracks: &[(i64, String, bool)],
    audio_lang: &str,
    prefs: &SubtitlePrefs,
) -> SubtitleChoice {
    match prefs.mode.as_str() {
        "None" => SubtitleChoice::Off,
        "OnlyForced" => subtitle_choice(only_forced_subtitle(tracks, &prefs.subtitle_lang)),
        "Always" => subtitle_choice(language_subtitle_or_first(tracks, &prefs.subtitle_lang)),
        "Smart" => subtitle_choice(smart_subtitle(tracks, audio_lang, &prefs.subtitle_lang)),
        "HearingImpaired" => {
            subtitle_choice(hearing_impaired_subtitle(tracks, &prefs.subtitle_lang))
        }
        // `""`, `"Default"`, and unrecognized modes leave the selection untouched.
        _ => SubtitleChoice::Leave,
    }
}

/// An active subtitle mode that resolves to no track turns subtitles off;
/// only `""`/`"Default"`/unknown modes leave the selection untouched.
fn subtitle_choice(track: Option<i64>) -> SubtitleChoice {
    track.map_or(SubtitleChoice::Off, SubtitleChoice::Track)
}

fn only_forced_subtitle(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, forced)| *forced && label_matches_lang(label, language))
        .or_else(|| tracks.iter().find(|(_, _, forced)| *forced))
        .map(|(id, _, _)| *id)
}

fn language_subtitle_or_first(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, _)| label_matches_lang(label, language))
        .or_else(|| tracks.first())
        .map(|(id, _, _)| *id)
}

fn smart_subtitle(
    tracks: &[(i64, String, bool)],
    audio_lang: &str,
    subtitle_lang: &str,
) -> Option<i64> {
    let audio_name = lang_code_to_name(audio_lang).to_lowercase();
    if !subtitle_lang.is_empty() && audio_name == subtitle_lang.to_lowercase() {
        None
    } else {
        language_subtitle_or_first(tracks, subtitle_lang)
    }
}

fn hearing_impaired_subtitle(tracks: &[(i64, String, bool)], language: &str) -> Option<i64> {
    tracks
        .iter()
        .find(|(_, label, _)| {
            let label = label.to_lowercase();
            label.contains("sdh") || label.contains(" cc") || label.contains("(cc)")
        })
        .or_else(|| {
            tracks
                .iter()
                .find(|(_, label, _)| label_matches_lang(label, language))
        })
        .or_else(|| tracks.first())
        .map(|(id, _, _)| *id)
}

pub(super) fn auto_select_tracks(
    mpv: &Mpv,
    status: &Arc<Mutex<PlayerStatus>>,
    prefs: &SubtitlePrefs,
) {
    refresh_tracks(mpv, status);
    let (audio_tracks, audio_id, audio_lang, sub_tracks) = {
        let status = status.lock().unwrap();
        (
            status.audio_tracks.clone(),
            status.audio_id,
            status.audio_lang.clone(),
            status.sub_tracks.clone(),
        )
    };
    let (audio, subtitle) = select_tracks(&audio_tracks, &sub_tracks, audio_id, &audio_lang, prefs);
    if let Some(id) = audio {
        let _ = mpv.set_property("aid", id);
        status.lock().unwrap().audio_id = id;
    }
    match subtitle {
        SubtitleChoice::Leave => {}
        SubtitleChoice::Off => {
            let _ = mpv.set_property("sid", "no".to_string());
            status.lock().unwrap().sub_id = 0;
        }
        SubtitleChoice::Track(id) => {
            let _ = mpv.set_property("sid", id);
            status.lock().unwrap().sub_id = id;
        }
    }
    refresh_tracks(mpv, status);
}

pub(super) fn refresh_tracks(mpv: &Mpv, status: &Arc<Mutex<PlayerStatus>>) {
    let count: i64 = match mpv.get_property("track-list/count") {
        Ok(n) => n,
        Err(_) => return,
    };
    let tracks = (0..count)
        .map(|index| {
            let property = |name: &str| format!("track-list/{index}/{name}");
            let kind = mpv.get_property(&property("type")).unwrap_or_default();
            let id = mpv.get_property(&property("id")).unwrap_or(index + 1);
            let lang = mpv.get_property(&property("lang")).unwrap_or_default();
            let title = mpv.get_property(&property("title")).unwrap_or_default();
            let codec = mpv.get_property(&property("codec")).unwrap_or_default();
            let selected = mpv.get_property(&property("selected")).unwrap_or(false);
            let channels = mpv
                .get_property(&property("demux-channel-count"))
                .unwrap_or(0);
            let forced = mpv.get_property(&property("forced")).unwrap_or(false);
            let stream_index = mpv
                .get_property(&property("ff-index"))
                .or_else(|_| mpv.get_property(&property("src-id")))
                .unwrap_or(-1);
            TrackInfo {
                kind,
                id,
                lang,
                title,
                codec,
                selected,
                channels,
                forced,
                stream_index,
            }
        })
        .collect::<Vec<_>>();
    let parsed = parse_tracks(&tracks);
    let mut status = status.lock().unwrap();
    status.audio_tracks = parsed.audio_tracks;
    status.sub_tracks = parsed.sub_tracks;
    status.sub_track_stream_indexes = parsed.sub_track_stream_indexes;
    status.audio_id = parsed.audio_id;
    status.audio_lang = parsed.audio_lang;
    status.sub_id = parsed.sub_id;
    status.sub_lang = parsed.sub_lang;
}

// ── Session infrastructure ────────────────────────────────────────────────────
