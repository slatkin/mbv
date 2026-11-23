//! The playback status-indicator cluster's neutral data: how the cluster is
//! styled, the already-resolved facts for the playing item, and the plain
//! text cluster both Clients paint. The colours are theme roles, never a
//! toolkit colour; each Client paints them its own way.

use mbv_theme::Role;

/// Which visual treatment to use for the status-indicator cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndicatorStyle {
    Brackets,
    Chips,
    Outlined,
    Dots,
    Pipes,
    #[default]
    KeyValue,
    Powerline,
}

impl std::str::FromStr for IndicatorStyle {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.trim().to_lowercase().as_str() {
            "brackets" => IndicatorStyle::Brackets,
            "chips" => IndicatorStyle::Chips,
            "outlined" => IndicatorStyle::Outlined,
            "dots" => IndicatorStyle::Dots,
            "pipes" => IndicatorStyle::Pipes,
            "" | "keyvalue" | "key_value" | "key-value" => IndicatorStyle::KeyValue,
            "powerline" => IndicatorStyle::Powerline,
            _ => return Err(()),
        })
    }
}

/// Compact booleans for resolved indicator semantics.
#[expect(
    clippy::struct_excessive_bools,
    reason = "four independent resolved indicator facts (resolution, audio-language dimming, audio-only, subtitle state) driving four separate render decisions, sourced from two different backends (design analysis, issue #804)"
)]
#[derive(Clone, Copy, Debug)]
pub struct IndicatorFlags {
    pub res_dim: bool,
    pub audio_dim: bool,
    pub audio_only: bool,
    pub sub_on: bool,
}

/// Already-resolved indicator values for the currently playing item.
#[derive(Debug)]
pub struct IndicatorData {
    pub res_label: String,
    pub audio_label: String,
    /// Display label for subtitles: the lang/CC abbreviation when on, "CC" when off.
    pub sub_label: String,
    pub flags: IndicatorFlags,
}

impl IndicatorData {
    /// The role the resolution label paints in: muted while unresolved.
    #[must_use]
    pub fn res_role(&self) -> Role {
        if self.flags.res_dim {
            Role::TextMuted
        } else {
            Role::IndicatorResolutionFg
        }
    }

    /// The role the audio label paints in: muted while unresolved.
    #[must_use]
    pub fn audio_role(&self) -> Role {
        if self.flags.audio_dim {
            Role::TextMuted
        } else {
            Role::IndicatorAudioFg
        }
    }

    /// The role the subtitle label paints in: muted while subtitles are off.
    #[must_use]
    pub fn sub_role(&self) -> Role {
        if self.flags.sub_on {
            Role::TextMetadata
        } else {
            Role::TextMuted
        }
    }
}

/// Short resolution pill: 2160+ -> 4K, 1440+ -> QHD, 1080+ -> FHD,
/// 720+ -> HD, anything lower -> SD.
#[must_use]
pub fn short_resolution_label(height: u64) -> &'static str {
    if height >= 2160 {
        "4K"
    } else if height >= 1440 {
        "QHD"
    } else if height >= 1080 {
        "FHD"
    } else if height >= 720 {
        "HD"
    } else {
        "SD"
    }
}

/// One status indicator of the queue band's plain text cluster: its
/// uppercased label and the foreground role its text paints in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusIndicator {
    pub label: String,
    pub role: Role,
}

/// The queue band's status-indicator cluster for `d`: the resolution, audio,
/// and subtitle labels the queue playback panel paints as plain text on its
/// controls row, uppercased, each in the role its state resolves — a dimmed
/// fact in `TextMuted`, the live subtitle label in `TextMetadata`. Audio-only
/// playback paints the resolution alone. The TUI's per-style treatments
/// (chips, brackets, dots, pipes) are terminal presentation the Clients share
/// through their own painters; the labels and their colour roles are the
/// content every Client paints from.
#[must_use]
pub fn status_indicator_items(d: &IndicatorData) -> Vec<StatusIndicator> {
    let mut items = vec![StatusIndicator {
        label: d.res_label.to_uppercase(),
        role: d.res_role(),
    }];
    if !d.flags.audio_only {
        items.push(StatusIndicator {
            label: d.audio_label.to_uppercase(),
            role: d.audio_role(),
        });
        items.push(StatusIndicator {
            label: d.sub_label.to_uppercase(),
            role: d.sub_role(),
        });
    }
    items
}
