//! The artist page's tracklist (gui-music-artist-page design D6): the artist's
//! tracks grouped by album, in the page's album order — one album label (the
//! album's name and release year) and then the album's tracks through the one
//! [`LibraryTracklist`] every track table shares. The label is a plain element
//! with no press handler: `CONTEXT.md` keeps a Group heading a non-selectable
//! label, and the album rows under the open artist hold the path to the album.
//! An album whose tracks have not loaded paints its label and no table, and it
//! is no part of a played sequence.
//!
//! The rows share the tracklist's interaction contract: the table keeps no
//! cursor, the hover Add appends only that track, and the Play control and
//! the double-click call `LibraryState::play_library_track`, which reads the
//! carried subject — under the artist page it sends `PlayArtistTrack`, so the
//! sequence runs through the page's albums and does not stop at the end of
//! the track's album.

use dioxus_native::prelude::*;

use crate::bridge::frame::library::GuiMusicAlbumFrame;
use crate::window::components::library_tracklist::LibraryTracklist;

/// The props of the artist page's tracklist (the `LibraryArtistTracklist`
/// props): the page's albums in the union's order, with their tracklists when
/// the cache holds them, and the id of the track playing at the playhead.
#[derive(Props, Debug, Clone, PartialEq)]
pub struct LibraryArtistTracklistProps {
    /// The artist page's albums (design D6): each paints one label, and one
    /// table while its tracks are loaded.
    pub albums: Vec<GuiMusicAlbumFrame>,
    /// The id of the track playing at the playhead, or `None` while idle.
    pub playing_track_id: Option<String>,
}

/// The album label (design D6): the album's name and release year, a plain
/// element with no press handler.
fn paint_album_label(album: &GuiMusicAlbumFrame) -> Element {
    let year = if album.year.is_empty() {
        String::new()
    } else {
        format!(" · {}", album.year)
    };
    rsx! {
        div {
            class: "flex items-center font-mono",
            style: "height: 30px; padding: 0 8px; margin-top: 12px; font-size: 12px; letter-spacing: 0.08em; color: var(--canvas-type-label);",
            "data-library-artist-album": "{album.album_id}",
            "{album.name}{year}"
        }
    }
}

/// The artist page's tracklist: one label per album, and the album's tracks
/// through the shared tracklist while they are loaded.
#[component]
pub fn LibraryArtistTracklist(props: LibraryArtistTracklistProps) -> Element {
    rsx! {
        div {
            class: "flex flex-col",
            "data-library-artist-tracklist": "",
            for album in props.albums.iter() {
                div {
                    key: "{album.album_id}",
                    class: "flex flex-col",
                    { paint_album_label(album) }
                    if let Some(tracks) = album.tracks.as_ref().filter(|tracks| !tracks.is_empty()) {
                        LibraryTracklist {
                            tracks: tracks.clone(),
                            playing_track_id: props.playing_track_id.clone(),
                        }
                    }
                }
            }
        }
    }
}
