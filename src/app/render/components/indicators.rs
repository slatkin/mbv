//! Status-indicator cluster treatments (resolution · audio · subtitles).
//!
//! Six interchangeable looks selected by config; the default is reverse-video
//! chips. Colors map onto the mbv palette: resolution = orange,
//! audio = blue (FOAM), subtitles = yellow (YELLOW) when on / dim when off.

use crate::app::palette;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

const ARROW: &str = "\u{E0B0}"; // powerline separator (needs a Nerd/Powerline font)

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
#[derive(Clone, Copy)]
pub struct IndicatorFlags {
    pub res_dim: bool,
    pub audio_dim: bool,
    pub audio_only: bool,
    pub sub_on: bool,
}

/// Already-resolved indicator values for the currently playing item.
pub struct IndicatorData {
    pub res_label: String,
    pub audio_label: String,
    /// Display label for subtitles: the lang/CC abbreviation when on, "CC" when off.
    pub sub_label: String,
    pub flags: IndicatorFlags,
}

impl IndicatorData {
    fn res_color(&self) -> Color {
        if self.flags.res_dim {
            palette::TEXT_MUTED
        } else {
            palette::INDICATOR_RESOLUTION_FG
        }
    }
    fn audio_color(&self) -> Color {
        if self.flags.audio_dim {
            palette::TEXT_MUTED
        } else {
            palette::INDICATOR_AUDIO_FG
        }
    }
    fn sub_color(&self) -> Color {
        if self.flags.sub_on {
            palette::TEXT_METADATA
        } else {
            palette::TEXT_MUTED
        }
    }
}

/// Short resolution pill: 2160+ -> 4K, 1440+ -> QHD, 1080+ -> FHD,
/// 720+ -> HD, anything lower -> SD.
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

/// Build the fully-styled indicator spans for the chosen treatment.
/// Widths are self-describing (each span carries its own padding), so callers
/// can measure with `span.content.width()` and right/center-align as needed.
pub fn indicator_spans(
    style: IndicatorStyle,
    d: &IndicatorData,
    use_nerd_fonts: bool,
) -> Vec<Span<'static>> {
    match style {
        IndicatorStyle::Chips => chips(d),
        IndicatorStyle::Brackets => brackets(d),
        IndicatorStyle::Outlined => outlined(d),
        IndicatorStyle::Dots => dots(d),
        IndicatorStyle::Pipes => pipes(d),
        IndicatorStyle::KeyValue => keyvalue(d),
        // Powerline needs a patched font; fall back to chips otherwise.
        IndicatorStyle::Powerline => {
            if use_nerd_fonts {
                powerline(d)
            } else {
                chips(d)
            }
        }
    }
}

fn bold(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

// --- Reverse-video chips (default) ---------------------------------------
fn chip(label: &str, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .bg(bg)
            .fg(palette::TEXT_ON_ACCENT)
            .add_modifier(Modifier::BOLD),
    )
}

fn chips(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = vec![chip(&d.res_label, d.res_color())];
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        out.push(chip(&d.audio_label, d.audio_color()));
        out.push(Span::raw(" "));
        if d.flags.sub_on {
            out.push(chip(&d.sub_label, d.sub_color()));
        } else {
            // Off: hollow/dim — no fill, dim text.
            out.push(Span::styled(
                format!(" {} ", d.sub_label),
                bold(palette::TEXT_MUTED),
            ));
        }
    }
    out
}

// --- Legacy brackets: [FHD] [en] [CC] -----------------------------------
fn bracket_group(label: &str, color: Color, out: &mut Vec<Span<'static>>) {
    let b = bold(palette::TEXT_STRONG);
    out.push(Span::styled("[", b));
    out.push(Span::styled(label.to_string(), bold(color)));
    out.push(Span::styled("]", b));
}

fn brackets(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    bracket_group(&d.res_label, d.res_color(), &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        bracket_group(&d.audio_label, d.audio_color(), &mut out);
        out.push(Span::raw(" "));
        bracket_group(&d.sub_label, d.sub_color(), &mut out);
    }
    out
}

// --- Outlined: thin side bars approximate a border -----------------------
fn outlined_group(label: &str, color: Color, out: &mut Vec<Span<'static>>) {
    out.push(Span::styled(
        format!("\u{258F}{label}\u{2595}"),
        Style::default().fg(color),
    ));
}

fn outlined(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    outlined_group(&d.res_label, d.res_color(), &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        outlined_group(&d.audio_label, d.audio_color(), &mut out);
        out.push(Span::raw(" "));
        outlined_group(&d.sub_label, d.sub_color(), &mut out);
    }
    out
}

// --- Status dots: ● label (uses filled glyph; opt-in) --------------------
fn dot_group(dot: &str, color: Color, label: &str, out: &mut Vec<Span<'static>>) {
    out.push(Span::styled(dot.to_string(), Style::default().fg(color)));
    out.push(Span::styled(
        format!(" {label}"),
        Style::default().fg(palette::TEXT_SECONDARY),
    ));
}

fn dots(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    dot_group("\u{25CF}", d.res_color(), &d.res_label, &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw("  "));
        dot_group("\u{25CF}", d.audio_color(), &d.audio_label, &mut out);
        out.push(Span::raw("  "));
        let dot = if d.flags.sub_on {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        dot_group(dot, d.sub_color(), &d.sub_label, &mut out);
    }
    out
}

// --- Pipe statusline: FHD │ en │ CC --------------------------------------
fn pipes(d: &IndicatorData) -> Vec<Span<'static>> {
    let sep = || Span::styled(" \u{2502} ", Style::default().fg(palette::TEXT_MUTED));
    let mut out = vec![Span::styled(
        d.res_label.clone(),
        Style::default().fg(d.res_color()),
    )];
    if !d.flags.audio_only {
        out.push(sep());
        out.push(Span::styled(
            d.audio_label.clone(),
            Style::default().fg(d.audio_color()),
        ));
        out.push(sep());
        out.push(Span::styled(
            d.sub_label.clone(),
            Style::default().fg(d.sub_color()),
        ));
    }
    out
}

// --- Labeled key·value: aac ⧸ en ⧸ CC, or FHD ⧸ en ⧸ CC -------------------
// The groups are separated by ` ⧸ ` and the set carries no leading or
// trailing space: the pill painter owns the padding around it, so a space
// here would double the pill's own pad.
fn keyvalue(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    // res_label already reads as a resolution (FHD/HD/SD/QHD/4K) — no suffix.
    out.push(Span::styled(d.res_label.clone(), bold(d.res_color())));
    if !d.flags.audio_only {
        out.push(Span::styled(
            " ⧸ ",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        out.push(Span::styled(d.audio_label.clone(), bold(d.audio_color())));
        out.push(Span::styled(
            " ⧸ ",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        out.push(Span::styled(d.sub_label.clone(), bold(d.sub_color())));
    }
    out
}

// --- Powerline segments (needs a patched font) ---------------------------
fn powerline(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut segs: Vec<(String, Color)> = vec![(d.res_label.clone(), d.res_color())];
    if !d.flags.audio_only {
        segs.push((d.audio_label.clone(), d.audio_color()));
        segs.push((d.sub_label.clone(), d.sub_color()));
    }
    let mut out = Vec::new();
    for i in 0..segs.len() {
        let (ref label, color) = segs[i];
        out.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(color)
                .fg(palette::TEXT_ON_ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        // Arrow: foreground = this segment's color, background = next segment's (or none).
        let next_bg = segs.get(i + 1).map(|(_, c)| *c);
        let mut arrow = Style::default().fg(color);
        if let Some(bg) = next_bg {
            arrow = arrow.bg(bg);
        }
        out.push(Span::styled(ARROW, arrow));
    }
    out
}
