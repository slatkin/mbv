use super::*;

pub fn parse_video_info(streams: &[Value]) -> String {
    let Some(s) = streams.iter().find(|s| s["Type"].as_str() == Some("Video")) else {
        return String::new();
    };
    let width = s["Width"].as_u64().unwrap_or(0);
    let height = s["Height"].as_u64().unwrap_or(0);
    let dim = width.max(height);
    let res = match dim {
        3840.. => "4K".to_string(),
        1920.. => "1080p".to_string(),
        1280.. => "720p".to_string(),
        720.. => "480p".to_string(),
        d if d > 0 => format!("{}p", height),
        _ => String::new(),
    };
    let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
    match (res.is_empty(), codec.is_empty()) {
        (false, false) => format!("{} {}", res, codec),
        (false, true) => res,
        (true, false) => codec,
        (true, true) => String::new(),
    }
}

fn audio_language_name(lang: &str) -> &'static str {
    match lang.to_lowercase().as_str() {
        "en" | "eng" => "English",
        "fr" | "fre" | "fra" => "French",
        "de" | "ger" | "deu" => "German",
        "es" | "spa" => "Spanish",
        "it" | "ita" => "Italian",
        "pt" | "por" => "Portuguese",
        "ja" | "jpn" => "Japanese",
        "ko" | "kor" => "Korean",
        "zh" | "chi" | "zho" => "Chinese",
        "ru" | "rus" => "Russian",
        "ar" | "ara" => "Arabic",
        "nl" | "nld" | "dut" => "Dutch",
        "sv" | "swe" => "Swedish",
        "no" | "nor" => "Norwegian",
        "da" | "dan" => "Danish",
        "fi" | "fin" => "Finnish",
        "pl" | "pol" => "Polish",
        "cs" | "cze" | "ces" => "Czech",
        "tr" | "tur" => "Turkish",
        _ => "",
    }
}

pub fn parse_audio_info(streams: &[Value]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for s in streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Audio"))
    {
        let lang = s["Language"].as_str().unwrap_or("");
        let lang_name = audio_language_name(lang);
        let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
        let layout = s["ChannelLayout"].as_str().unwrap_or("");
        let layout_str = match layout {
            "mono" => "Mono",
            "stereo" => "Stereo",
            "5.1" => "5.1",
            "7.1" => "7.1",
            other if !other.is_empty() => other,
            _ => "",
        };
        let track: Vec<&str> = [lang_name, &codec, layout_str]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect();
        if !track.is_empty() {
            parts.push(track.join(" "));
        }
    }
    parts.join("  |  ")
}

pub fn parse_session_media_info(streams: &[Value]) -> SessionMediaInfo {
    let video = streams.iter().find(|s| s["Type"].as_str() == Some("Video"));
    let audio_only = video.is_none();
    let video_label = if audio_only {
        parse_audio_info(streams)
            .split("  |  ")
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        parse_video_info(streams)
    };

    let audio_streams = streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Audio"))
        .filter_map(|s| {
            s.get("Index")?;
            let index = s["Index"].as_i64().unwrap_or(0);
            let language = s["Language"].as_str().unwrap_or("").to_string();
            let label = {
                let lang_name = audio_language_name(&language);
                let codec = s["Codec"].as_str().unwrap_or("").to_uppercase();
                let layout = s["ChannelLayout"].as_str().unwrap_or("");
                let layout_str = match layout {
                    "mono" => "Mono",
                    "stereo" => "Stereo",
                    "5.1" => "5.1",
                    "7.1" => "7.1",
                    other if !other.is_empty() => other,
                    _ => "",
                };
                let title = s["DisplayTitle"]
                    .as_str()
                    .or_else(|| s["Title"].as_str())
                    .unwrap_or("");
                let pieces: Vec<&str> = [lang_name, &codec, layout_str]
                    .iter()
                    .filter(|part| !part.is_empty())
                    .copied()
                    .collect();
                if !pieces.is_empty() {
                    pieces.join(" ")
                } else if !title.is_empty() {
                    title.to_string()
                } else if !language.is_empty() {
                    language.to_uppercase()
                } else {
                    format!("#{index}")
                }
            };
            Some(SessionAudioStream {
                index,
                label,
                language,
            })
        })
        .collect();

    let subtitle_streams = streams
        .iter()
        .filter(|s| s["Type"].as_str() == Some("Subtitle"))
        .filter_map(|s| {
            let index = s["Index"].as_i64().unwrap_or(-1);
            if index < 0 {
                return None;
            }
            let language = s["Language"].as_str().unwrap_or("").to_string();
            let forced = s["IsForced"].as_bool().unwrap_or(false);
            let title = s["DisplayTitle"]
                .as_str()
                .or_else(|| s["Title"].as_str())
                .unwrap_or("");
            let lang_name = audio_language_name(&language);
            let base = if !title.is_empty() {
                title.to_string()
            } else if !lang_name.is_empty() {
                lang_name.to_string()
            } else if !language.is_empty() {
                language.to_uppercase()
            } else {
                format!("#{index}")
            };
            let label = if forced {
                format!("{base} (Forced)")
            } else {
                base
            };
            Some(SessionSubtitleStream {
                index,
                label,
                language,
                forced,
            })
        })
        .collect();

    SessionMediaInfo {
        video_label,
        audio_only,
        audio_streams,
        subtitle_streams,
    }
}

/// The single raw-JSON-to-`EmbyItem` constructor. `pub` so the app crate's
/// tests parse recorded item JSON (task 5.3's fixtures) the way the live
/// parse path does (task 5.4's artwork-policy tests).
pub fn parse_item(raw: &Value) -> EmbyItem {
    let ud = raw.get("UserData").unwrap_or(&Value::Null);
    let item_type = raw["Type"].as_str().unwrap_or("").to_string();
    let is_folder = raw["IsFolder"].as_bool().unwrap_or(false)
        || matches!(
            item_type.as_str(),
            "CollectionFolder"
                | "Channel"
                | "Series"
                | "Season"
                | "MusicArtist"
                | "MusicAlbum"
                | "BoxSet"
                | "Folder"
        );
    let total_count = if item_type == "Series" {
        raw["RecursiveItemCount"].as_u64().unwrap_or(0) as u32
    } else {
        raw["ChildCount"].as_u64().unwrap_or(0) as u32
    };
    EmbyItem {
        id: raw["Id"].as_str().unwrap_or("").to_string(),
        name: raw["Name"].as_str().unwrap_or("").to_string(),
        item_type,
        is_folder,
        child_count: raw["ChildCount"]
            .as_u64()
            .and_then(|count| u32::try_from(count).ok()),
        media_type: raw["MediaType"].as_str().unwrap_or("").to_string(),
        collection_type: raw["CollectionType"].as_str().unwrap_or("").to_string(),
        runtime_ticks: raw["RunTimeTicks"].as_i64().unwrap_or(0),
        played: ud["Played"].as_bool().unwrap_or(false),
        playback_position_ticks: ud["PlaybackPositionTicks"].as_i64().unwrap_or(0),
        series_id: raw["SeriesId"].as_str().unwrap_or("").to_string(),
        series_name: raw["SeriesName"].as_str().unwrap_or("").to_string(),
        album_id: raw["AlbumId"].as_str().unwrap_or("").to_string(),
        album: raw["Album"].as_str().unwrap_or("").to_string(),
        index_number: raw["IndexNumber"].as_i64().unwrap_or(0),
        parent_index_number: raw["ParentIndexNumber"].as_i64().unwrap_or(0),
        unplayed_item_count: ud["UnplayedItemCount"].as_u64().unwrap_or(0) as u32,
        path: raw["Path"].as_str().unwrap_or("").to_string(),
        artist: raw["AlbumArtist"]
            .as_str()
            .or_else(|| raw["Artists"].get(0).and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string(),
        artist_items: raw["ArtistItems"]
            .as_array()
            .map(|pairs| {
                pairs
                    .iter()
                    .filter_map(|pair| {
                        let name = pair["Name"].as_str()?;
                        let id = pair["Id"].as_str()?;
                        Some(EmbyArtistRef {
                            name: name.to_string(),
                            id: id.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        sort_name: raw["SortName"].as_str().unwrap_or("").to_string(),
        production_year: raw["ProductionYear"]
            .as_u64()
            .or_else(|| raw["Year"].as_u64())
            .unwrap_or(0) as u32,
        end_year: raw["EndDate"]
            .as_str()
            .and_then(|s| s.get(..4))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        overview: decode_entities(raw["Overview"].as_str().unwrap_or("")),
        premiere_date: raw["PremiereDate"]
            .as_str()
            .and_then(|s| s.get(..10))
            .map(|s| s.to_string())
            .unwrap_or_default(),
        date_added: raw["DateCreated"].as_str().unwrap_or_default().to_string(),
        total_count,
        container: raw["Container"].as_str().unwrap_or("").to_string(),
        genres: raw["Genres"]
            .as_array()
            .map(|genres| {
                genres
                    .iter()
                    .filter_map(|genre| genre.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        people: raw["People"]
            .as_array()
            .map(|people| {
                people
                    .iter()
                    .map(|person| EmbyPerson {
                        name: person["Name"].as_str().unwrap_or("").to_string(),
                        role: person["Role"].as_str().unwrap_or("").to_string(),
                        kind: person["Type"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        external_urls: raw["ExternalUrls"]
            .as_array()
            .map(|links| {
                links
                    .iter()
                    .map(|link| EmbyLink {
                        name: link["Name"].as_str().unwrap_or("").to_string(),
                        url: link["Url"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        video_info: raw["MediaStreams"]
            .as_array()
            .map(|s| parse_video_info(s))
            .unwrap_or_default(),
        playlist_item_id: raw["PlaylistItemId"].as_str().unwrap_or("").to_string(),
        image_tags: parse_image_tags(raw),
        audio_info: raw["MediaStreams"]
            .as_array()
            .map(|s| parse_audio_info(s))
            .unwrap_or_default(),
    }
}

/// Reads one item's declared image availability (task 5.3): `ImageTags`
/// (`Thumb`, `Primary`), `BackdropImageTags`, and — for episodes — the
/// series' thumb/backdrop tags. Every missing member defaults to empty.
fn parse_image_tags(raw: &Value) -> EmbyImageTags {
    let tags = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .map(|tags| {
                tags.iter()
                    .filter_map(|tag| tag.as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    EmbyImageTags {
        thumb: raw["ImageTags"]["Thumb"].as_str().unwrap_or("").to_string(),
        logo: raw["ImageTags"]["Logo"].as_str().unwrap_or("").to_string(),
        primary: raw["ImageTags"]["Primary"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        backdrops: tags(&raw["BackdropImageTags"]),
        series_thumb: raw["ParentThumbImageTag"]
            .as_str()
            .or_else(|| raw["SeriesThumbImageTag"].as_str())
            .unwrap_or("")
            .to_string(),
        series_backdrops: tags(&raw["ParentBackdropImageTags"]),
    }
}

pub(crate) fn load_cached_token() -> Option<(String, String, String)> {
    let path = crate::config::token_cache_path();
    let text = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let token = v["token"].as_str()?.to_string();
    let user_id = v["user_id"].as_str()?.to_string();
    if token.is_empty() || user_id.is_empty() {
        return None;
    }
    let server_url = v["server_url"].as_str().unwrap_or("").to_string();
    Some((server_url, token, user_id))
}

pub fn clear_cached_token() {
    let _ = std::fs::remove_file(crate::config::token_cache_path());
}

#[cfg(test)]
pub fn save_cached_token(server_url: &str, token: &str, user_id: &str) {
    let path = crate::config::token_cache_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let json = serde_json::json!({"server_url": server_url, "token": token, "user_id": user_id});
    let _ = std::fs::write(&path, json.to_string());
    // Restrict token file to owner-only to protect credentials.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}

