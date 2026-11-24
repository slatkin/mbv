//! The Service mark a GUI Library destination's header paints (design D4 of
//! `gui-library-header-service-marks`): the brand mark that names the Service,
//! or the Feed platform the checked subscriptions share.
//!
//! The paths are simple-icons' own (<https://simpleicons.org>, CC0, release
//! 16.38.0). Most marks paint their brand hex; Patreon and the Lucide RSS mark
//! paint the theme's heading colour, because the brand black does not show on a
//! dark theme. The colour goes on the svg's computed `color` and the path
//! fills with `currentColor`, because usvg cannot resolve a `var()` in a
//! `fill` attribute (`window/icons.rs` holds the reason).
//!
//! This file is the one brand-mark table in `window/` (issue #1043): the
//! Emby path and hex serve this painter and `brand_icons.rs`'s
//! playback-target chooser rows from one copy, and the Cast path and hex
//! serve the chooser's Cast row. The mpv mark is not a third-party brand,
//! so `brand_icons.rs` keeps its path.

use dioxus_native::prelude::*;
use mbv_feed::FeedPlatform;

use crate::window::icons::Rss;
use crate::window::theme::canvas_var;

/// The Emby path, on a 24 × 24 view box: the playback-target chooser's Emby
/// row and this mark paint the same mark.
pub(crate) const EMBY_PATH: &str = "M11.041 0c-.007 0-1.456 1.43-3.219 3.176L4.615 6.352l.512.513.512.512-2.819 2.791L0 12.961l1.83 1.848c1.006 1.016 2.438 2.46 3.182 3.209l1.351 1.359.508-.496c.28-.273.515-.498.524-.498.008 0 1.266 1.264 2.794 2.808L12.97 24l.187-.182c.23-.225 5.007-4.95 5.717-5.656l.52-.516-.502-.513c-.276-.282-.5-.52-.496-.53.003-.009 1.264-1.26 2.802-2.783 1.538-1.522 2.8-2.776 2.803-2.785.005-.012-3.617-3.684-6.107-6.193L17.65 4.6l-.505.505c-.279.278-.517.501-.53.497-.013-.005-1.27-1.267-2.793-2.805A449.655 449.655 0 0011.041 0zM9.223 7.367c.091.038 7.951 4.608 7.957 4.627.003.013-1.781 1.056-3.965 2.32a999.898 999.898 0 01-3.996 2.307c-.019.006-.026-1.266-.026-4.629 0-3.7.007-4.634.03-4.625Z";

/// The Emby brand hex: the Library header's Emby mark and the chooser's Emby
/// row paint one copy.
pub(crate) const EMBY_COLOR: &str = "#52B54B";

/// The Google Cast path, on a 24 × 24 view box: the playback-target chooser's
/// Cast row paints it from this one copy.
pub(crate) const CAST_PATH: &str = "M0 18.5455v3.2727h3.2727c0-1.811-1.4618-3.2727-3.2727-3.2727zm0-4.3637v2.1818c3.011 0 5.4545 2.4437 5.4545 5.4546h2.1819c0-4.2218-3.4146-7.6364-7.6364-7.6364zm0-4.3636V12c5.4218 0 9.8182 4.3964 9.8182 9.8182H12c0-6.6327-5.3782-12-12-12zm21.8182-7.6364H2.1818C.9818 2.1818 0 3.1636 0 4.3636v3.2728h2.1818V4.3636h19.6364v15.2728h-7.6364v2.1818h7.6364c1.2 0 2.1818-.9818 2.1818-2.1818V4.3636c0-1.2-.9818-2.1818-2.1818-2.1818Z";

/// The Google Cast brand hex, on the playback-target chooser's Cast row.
pub(crate) const CAST_COLOR: &str = "#4285F4";

const AUDIOBOOKSHELF_PATH: &str = "M12 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0Zm-.023.402A11.598 11.598 0 0 1 23.575 12a11.598 11.598 0 0 1-11.598 11.598A11.598 11.598 0 0 1 .378 12 11.598 11.598 0 0 1 11.977.402Zm0 1.776a7.093 7.093 0 0 0-7.092 7.093v1.536a6.395 6.395 0 0 0-.439.33.35.35 0 0 0-.126.27v1.84a.36.36 0 0 0 .126.272c.22.182.722.564 1.504.956v.179c0 .483.31.873.694.873.384 0 .694-.392.694-.873v-4.415c0-.483-.31-.873-.694-.873-.369 0-.67.359-.694.812h-.002v-.91a6.027 6.027 0 1 1 12.054.003v.91c-.025-.454-.326-.813-.695-.813-.384 0-.694.391-.694.873v4.415c0 .483.31.873.694.873.384 0 .695-.392.695-.873v-.179a7.964 7.964 0 0 0 1.503-.956.35.35 0 0 0 .126-.272v-1.843a.342.342 0 0 0-.124-.27 5.932 5.932 0 0 0-.438-.329V9.271a7.093 7.093 0 0 0-7.092-7.093zm-3.34 5.548a.84.84 0 0 0-.84.84v9.405c0 .464.376.84.84.84h.866a.84.84 0 0 0 .84-.84V8.566a.84.84 0 0 0-.84-.84Zm2.905 0a.84.84 0 0 0-.84.84v9.405c0 .464.377.84.84.84h.867a.84.84 0 0 0 .84-.84V8.566a.84.84 0 0 0-.84-.84zm2.908 0a.84.84 0 0 0-.84.84v9.405c0 .464.376.84.84.84h.867a.84.84 0 0 0 .84-.84V8.566a.84.84 0 0 0-.84-.84zM8.112 9.983h1.915v.2H8.112Zm2.906 0h1.915v.2h-1.915Zm2.908 0h1.915v.2h-1.915zm-7.58 9.119a.633.633 0 0 0 0 1.265h11.26a.632.632 0 0 0 0-1.265z";

const YOUTUBE_PATH: &str = "M23.498 6.186a3.016 3.016 0 0 0-2.122-2.136C19.505 3.545 12 3.545 12 3.545s-7.505 0-9.377.505A3.017 3.017 0 0 0 .502 6.186C0 8.07 0 12 0 12s0 3.93.502 5.814a3.016 3.016 0 0 0 2.122 2.136c1.871.505 9.376.505 9.376.505s7.505 0 9.377-.505a3.015 3.015 0 0 0 2.122-2.136C24 15.93 24 12 24 12s0-3.93-.502-5.814zM9.545 15.568V8.432L15.818 12l-6.273 3.568z";

const SOUNDCLOUD_PATH: &str = "M23.999 14.165c-.052 1.796-1.612 3.169-3.4 3.169h-8.18a.68.68 0 0 1-.675-.683V7.862a.747.747 0 0 1 .452-.724s.75-.513 2.333-.513a5.364 5.364 0 0 1 2.763.755 5.433 5.433 0 0 1 2.57 3.54c.282-.08.574-.121.868-.12.884 0 1.73.358 2.347.992s.948 1.49.922 2.373ZM10.721 8.421c.247 2.98.427 5.697 0 8.672a.264.264 0 0 1-.53 0c-.395-2.946-.22-5.718 0-8.672a.264.264 0 0 1 .53 0ZM9.072 9.448c.285 2.659.37 4.986-.006 7.655a.277.277 0 0 1-.55 0c-.331-2.63-.256-5.02 0-7.655a.277.277 0 0 1 .556 0Zm-1.663-.257c.27 2.726.39 5.171 0 7.904a.266.266 0 0 1-.532 0c-.38-2.69-.257-5.21 0-7.904a.266.266 0 0 1 .532 0Zm-1.647.77a26.108 26.108 0 0 1-.008 7.147.272.272 0 0 1-.542 0 27.955 27.955 0 0 1 0-7.147.275.275 0 0 1 .55 0Zm-1.67 1.769c.421 1.865.228 3.5-.029 5.388a.257.257 0 0 1-.514 0c-.21-1.858-.398-3.549 0-5.389a.272.272 0 0 1 .543 0Zm-1.655-.273c.388 1.897.26 3.508-.01 5.412-.026.28-.514.283-.54 0-.244-1.878-.347-3.54-.01-5.412a.283.283 0 0 1 .56 0Zm-1.668.911c.4 1.268.257 2.292-.026 3.572a.257.257 0 0 1-.514 0c-.241-1.262-.354-2.312-.023-3.572a.283.283 0 0 1 .563 0Z";

const SPREAKER_PATH: &str = "M14.615 0l-5.64 6.54L.529 4.718l8.68 7.372-8.537 7.463 8.411-1.984L14.843 24l.71-8.601 7.918-3.483-7.963-3.33L14.621 0h-.006z";

const AUDIOBOOM_PATH: &str = "M12 24C5.373 24 0 18.627 0 12S5.373 0 12 0s12 5.373 12 12-5.373 12-12 12zM7.425 3.214c-.621 0-1.125.503-1.125 1.124v6a1.124 1.124 0 0 0 2.25 0v-6c0-.62-.504-1.124-1.125-1.124zm0 9.314c-.621 0-1.125.503-1.125 1.125v6a1.124 1.124 0 0 0 2.25 0v-6c0-.622-.504-1.125-1.125-1.125zm4.152-6.856c-.621 0-1.125.504-1.125 1.125v10.388a1.124 1.124 0 0 0 2.25 0V6.797c0-.621-.504-1.125-1.125-1.125zm4.151 6.856c-.62 0-1.124.503-1.124 1.125v1.056a1.124 1.124 0 1 0 2.249 0v-1.056c0-.622-.504-1.125-1.125-1.125zm0-4.37c-.62 0-1.124.503-1.124 1.124v1.056a1.124 1.124 0 0 0 2.249 0V9.282c0-.62-.504-1.124-1.125-1.124zm4.152 2.422c-.62 0-1.124.503-1.124 1.124v.574a1.124 1.124 0 1 0 2.249 0v-.574c0-.62-.504-1.124-1.125-1.124Z";

const PATREON_PATH: &str = "M22.957 7.21c-.004-3.064-2.391-5.576-5.191-6.482-3.478-1.125-8.064-.962-11.384.604C2.357 3.231 1.093 7.391 1.046 11.54c-.039 3.411.302 12.396 5.369 12.46 3.765.047 4.326-4.804 6.068-7.141 1.24-1.662 2.836-2.132 4.801-2.618 3.376-.836 5.678-3.501 5.673-7.031Z";

const SUBSTACK_PATH: &str = "M22.539 8.242H1.46V5.406h21.08v2.836zM1.46 10.812V24L12 18.11 22.54 24V10.812H1.46zM22.54 0H1.46v2.836h21.08V0z";

const SPOTIFY_PATH: &str = "M12 0C5.4 0 0 5.4 0 12s5.4 12 12 12 12-5.4 12-12S18.66 0 12 0zm5.521 17.34c-.24.359-.66.48-1.021.24-2.82-1.74-6.36-2.101-10.561-1.141-.418.122-.779-.179-.899-.539-.12-.421.18-.78.54-.9 4.56-1.021 8.52-.6 11.64 1.32.42.18.479.659.301 1.02zm1.44-3.3c-.301.42-.841.6-1.262.3-3.239-1.98-8.159-2.58-11.939-1.38-.479.12-1.02-.12-1.14-.6-.12-.48.12-1.021.6-1.141C9.6 9.9 15 10.561 18.72 12.84c.361.181.54.78.241 1.2zm.12-3.36C15.24 8.4 8.82 8.16 5.16 9.301c-.6.179-1.2-.181-1.38-.721-.18-.601.18-1.2.72-1.381 4.26-1.26 11.28-1.02 15.721 1.621.539.3.719 1.02.419 1.56-.299.421-1.02.599-1.559.3z";

const ODYSEE_PATH: &str = "M11.965 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12 12 12 0 0 0-3.209-8.167 7.272 7.272 0 0 1-.799 3.218c-.548.957-2.281 2.77-3.264 3.699a.723.723 0 0 0 .056 1.104c.996.74 2.658 2.151 2.788 3.422.176 1.835 1.6 4.02 1.675 4.159a.67.67 0 0 1-.105.327 12.067 12.067 0 0 1-2.03 1.898 2.435 2.435 0 0 1-.807.126c-1.944-.04-1.526-1.866-1.712-2.905s-.78-3.085-2.716-2.788c0 0 .484 4.243-1.489 5.546s-5.843 2.27-6.55-.408 2.46-2.384 2.684-2.384c.223 0 2.233-.632 1.267-2.53-.967-1.898-2.01-3.5-2.01-3.5a11.37 11.37 0 0 0-2.735 1.285 5.42 5.42 0 0 0-1.061.82c-1.065 1.104-2.19 1.713-2.954 1.358a1.368 1.368 0 0 1-.32-.221A11.926 11.926 0 0 1 .1 13.503c.43-.641 2.082-2.038 3.696-2.906 1.304-.702 2.737-.988 3.118-1.355-.671-2.235-1.882-5.703.832-7.33C9.881.634 12.69-.142 13.77 2.958c1.08 3.1.802 3.796 1.267 3.796.465 0 1.608.223 2.09-1.75.356-1.445.574-2.685 1.379-3.087A12 12 0 0 0 12 0a12 12 0 0 0-.035 0zm-.498 2.125c-.353-.019-.78.05-1.303.224 0 0-1.895.52-1.749 2.53.13 1.777 1.08 2.753 3.053 2.01 1.972-.737 2.31-1.264 1.824-2.753-.364-1.117-.765-1.956-1.825-2.011zm.48.726a.249.249 0 0 1 .183.07 1.58 1.58 0 0 1 .232.346.253.253 0 0 1-.197.37.253.253 0 0 1-.233-.131 1.559 1.559 0 0 0-.148-.232.256.256 0 0 1 0-.353.249.249 0 0 1 .163-.07zm3.529.152a.22.22 0 0 1 .192.103.22.22 0 0 1-.07.305.22.22 0 1 1-.122-.408zM12.609 4.27a.253.253 0 0 1 .227.133 1.5 1.5 0 0 1 .102.982.253.253 0 0 1-.246.205h-.049a.253.253 0 0 1-.2-.295 1.013 1.013 0 0 0-.071-.697.253.253 0 0 1 .237-.328zm-8.035.552a.12.12 0 0 1 .029.003.12.12 0 1 1-.147.091.12.12 0 0 1 .118-.094zm15.77 3.419.273.539.593.12-.539.275-.123.592-.272-.539-.592-.123.538-.272zm-17.767.535a.22.22 0 0 1 .193.104.22.22 0 0 1-.07.304.22.22 0 1 1-.123-.408zm16.004 2.79a.258.258 0 0 1 .062.007.258.258 0 1 1-.31.195.258.258 0 0 1 .248-.202zM4.914 16.115a.17.17 0 0 1 .165.14.17.17 0 0 1-.137.197.17.17 0 1 1-.028-.337zm9.769 4.094a.2.2 0 0 1 .036.004.2.2 0 1 1-.233.15.2.2 0 0 1 .197-.154z";

const PEERTUBE_PATH: &str = "M12 6.545v10.91L20.727 12M3.273 12v12L12 17.455M3.273 0v12L12 6.545";

const INTERNET_ARCHIVE_PATH: &str = "M22.667 22.884V24H1.333v-1.116zm-.842-1.675v1.396H2.175v-1.396zM4.233 6.14l.234.118.118 1.882.117 3.058v2.941l-.117 3.666-.02 2.47-.332.098H3.062l-.352-.098-.136-2.47-.118-3.646v-2.941l.118-3.078.107-1.892.244-.107zm16.842 0l.235.118.117 1.882.117 3.058v2.941l-.117 3.666-.02 2.47-.332.098h-1.171l-.352-.098-.137-2.47-.117-3.646v-2.941l.117-3.078.108-1.892.244-.107zm-11.79 0l.235.118.117 1.882.117 3.058v2.941l-.117 3.666-.02 2.47-.331.098H8.114l-.352-.098-.136-2.47-.117-3.646v-2.941l.117-3.078.107-1.892.244-.107zm6.457 0l.234.118.117 1.882.118 3.058v2.941l-.118 3.666-.019 2.47-.332.098H14.57l-.351-.098-.137-2.47-.117-3.646v-2.941l.117-3.078.108-1.892.244-.107zm6.083-2.511V5.58H2.175V3.628zM11.798 0l10.307 2.347-.413.723H1.951l-.618-.587Z";

/// A Service or Feed platform mark, as the GUI Library header paints it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMark {
    /// Emby.
    Emby,
    /// Audiobookshelf.
    Audiobookshelf,
    /// A generic feed, painted with the Lucide `rss` icon.
    Rss,
    /// A feed on a known Feed platform.
    Feed(FeedPlatform),
}

impl ServiceMark {
    /// The Feeds tab's mark: the platform the checked subscriptions share,
    /// else RSS.
    #[must_use]
    pub fn feeds(platform: Option<FeedPlatform>) -> Self {
        platform.map_or(Self::Rss, Self::Feed)
    }

    /// Everything this mark paints.
    fn paint(self) -> MarkPaint {
        match self {
            Self::Emby => MarkPaint::brand("emby", "Emby", EMBY_COLOR, EMBY_PATH),
            Self::Audiobookshelf => MarkPaint::brand(
                "audiobookshelf",
                "Audiobookshelf",
                "#82612C",
                AUDIOBOOKSHELF_PATH,
            ),
            Self::Rss => MarkPaint::new("rss", "RSS", Fill::Lucide),
            Self::Feed(platform) => match platform {
                FeedPlatform::YouTube => {
                    MarkPaint::brand("youtube", "YouTube", "#FF0000", YOUTUBE_PATH)
                }
                FeedPlatform::SoundCloud => {
                    MarkPaint::brand("soundcloud", "SoundCloud", "#FF5500", SOUNDCLOUD_PATH)
                }
                FeedPlatform::Spreaker => {
                    MarkPaint::brand("spreaker", "Spreaker", "#F5C300", SPREAKER_PATH)
                }
                FeedPlatform::Audioboom => {
                    MarkPaint::brand("audioboom", "Audioboom", "#007CE2", AUDIOBOOM_PATH)
                }
                FeedPlatform::Patreon => {
                    MarkPaint::new("patreon", "Patreon", Fill::Heading(PATREON_PATH))
                }
                FeedPlatform::Substack => {
                    MarkPaint::brand("substack", "Substack", "#FF6719", SUBSTACK_PATH)
                }
                FeedPlatform::Spotify => {
                    MarkPaint::brand("spotify", "Spotify", "#1ED760", SPOTIFY_PATH)
                }
                FeedPlatform::Odysee => {
                    MarkPaint::brand("odysee", "Odysee", "#EF1970", ODYSEE_PATH)
                }
                FeedPlatform::PeerTube => {
                    MarkPaint::brand("peertube", "PeerTube", "#F1680D", PEERTUBE_PATH)
                }
                FeedPlatform::InternetArchive => MarkPaint::brand(
                    "internetarchive",
                    "Internet Archive",
                    "#666666",
                    INTERNET_ARCHIVE_PATH,
                ),
            },
        }
    }
}

/// What one mark paints: the simple-icons slug the `data-library-list-mark`
/// hook carries, the accessible name, and how the glyph fills.
struct MarkPaint {
    /// The simple-icons slug.
    slug: &'static str,
    /// The Service or platform name, as the svg's `aria-label`.
    label: &'static str,
    /// How the glyph paints.
    fill: Fill,
}

/// How a mark's glyph fills.
enum Fill {
    /// A simple-icons path in its brand hex.
    Brand {
        color: &'static str,
        path: &'static str,
    },
    /// A simple-icons path in the theme's heading colour.
    Heading(&'static str),
    /// The Lucide RSS icon in the theme's heading colour, painted from
    /// `window/icons.rs`.
    Lucide,
}

impl MarkPaint {
    fn new(slug: &'static str, label: &'static str, fill: Fill) -> Self {
        Self { slug, label, fill }
    }

    /// A simple-icons mark in its brand `color`.
    fn brand(
        slug: &'static str,
        label: &'static str,
        color: &'static str,
        path: &'static str,
    ) -> Self {
        Self::new(slug, label, Fill::Brand { color, path })
    }
}

/// The mark of `mark`, `size` CSS pixels square.
#[component]
pub fn ServiceMarkIcon(mark: ServiceMark, #[props(default = 18)] size: u16) -> Element {
    let MarkPaint { slug, label, fill } = mark.paint();
    let (color, path) = match fill {
        Fill::Brand { color, path } => (color.to_owned(), Some(path)),
        Fill::Heading(path) => (canvas_var("heading"), Some(path)),
        Fill::Lucide => (canvas_var("heading"), None),
    };
    match path {
        Some(path) => rsx! {
            svg {
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 24 24",
                width: "{size}",
                height: "{size}",
                fill: "currentColor",
                style: "color: {color};",
                role: "img",
                "aria-label": label,
                "data-library-list-mark": slug,
                path { d: path }
            }
        },
        // The Lucide shell takes no extra attributes, so the span above the
        // icon carries the hook and the accessible name: one hook per mark,
        // whichever painter draws it.
        None => rsx! {
            span {
                class: "inline-flex",
                role: "img",
                "aria-label": label,
                "data-library-list-mark": slug,
                Rss { size, color }
            }
        },
    }
}
