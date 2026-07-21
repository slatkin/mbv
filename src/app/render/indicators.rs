//! Status-indicator cluster treatments (resolution · audio · subtitles).
//!
//! Six interchangeable looks selected by config; the default is reverse-video
//! chips. Colors map onto the mbv palette: resolution = green (IRIS),
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

/// Already-resolved indicator values for the currently playing item.
pub struct IndicatorData {
    pub res_label: String,
    pub res_dim: bool,
    pub audio_label: String,
    pub audio_dim: bool,
    pub audio_only: bool,
    /// Display label for subtitles. Empty = subs off. Non-empty = on, with this lang/label.
    pub sub_label: String,
}

impl IndicatorData {
    fn res_color(&self) -> Color {
        if self.res_dim {
            palette::MUTED
        } else {
            palette::IRIS
        }
    }
    fn audio_color(&self) -> Color {
        if self.audio_dim {
            palette::MUTED
        } else {
            palette::GREEN
        }
    }
    fn sub_color(&self) -> Color {
        if !self.sub_label.is_empty() {
            palette::YELLOW
        } else {
            palette::MUTED
        }
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
            .fg(palette::BASE)
            .add_modifier(Modifier::BOLD),
    )
}

fn chips(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = vec![chip(&d.res_label, d.res_color())];
    if !d.audio_only {
        out.push(Span::raw(" "));
        out.push(chip(&d.audio_label, d.audio_color()));
        out.push(Span::raw(" "));
        let sub_display = if d.sub_label.is_empty() {
            "CC"
        } else {
            &d.sub_label
        };
        if !d.sub_label.is_empty() {
            out.push(chip(sub_display, palette::YELLOW));
        } else {
            // Off: hollow/dim — no fill, dim text.
            out.push(Span::styled(
                format!(" {sub_display} "),
                bold(palette::MUTED),
            ));
        }
    }
    out
}

// --- Legacy brackets: [720p] [en] [CC] -----------------------------------
fn bracket_group(label: &str, color: Color, out: &mut Vec<Span<'static>>) {
    let b = bold(palette::WHITE);
    out.push(Span::styled("[", b));
    out.push(Span::styled(label.to_string(), bold(color)));
    out.push(Span::styled("]", b));
}

fn brackets(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    bracket_group(&d.res_label, d.res_color(), &mut out);
    if !d.audio_only {
        out.push(Span::raw(" "));
        bracket_group(&d.audio_label, d.audio_color(), &mut out);
        out.push(Span::raw(" "));
        let sub_display = if d.sub_label.is_empty() {
            "CC"
        } else {
            &d.sub_label
        };
        bracket_group(sub_display, d.sub_color(), &mut out);
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
    if !d.audio_only {
        out.push(Span::raw(" "));
        outlined_group(&d.audio_label, d.audio_color(), &mut out);
        out.push(Span::raw(" "));
        let sub_display = if d.sub_label.is_empty() {
            "CC"
        } else {
            &d.sub_label
        };
        outlined_group(sub_display, d.sub_color(), &mut out);
    }
    out
}

// --- Status dots: ● label (uses filled glyph; opt-in) --------------------
fn dot_group(dot: &str, color: Color, label: &str, out: &mut Vec<Span<'static>>) {
    out.push(Span::styled(dot.to_string(), Style::default().fg(color)));
    out.push(Span::styled(
        format!(" {label}"),
        Style::default().fg(palette::SUBTLE),
    ));
}

fn dots(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    dot_group("\u{25CF}", d.res_color(), &d.res_label, &mut out);
    if !d.audio_only {
        out.push(Span::raw("  "));
        dot_group("\u{25CF}", d.audio_color(), &d.audio_label, &mut out);
        out.push(Span::raw("  "));
        let sub_display = if d.sub_label.is_empty() {
            "CC"
        } else {
            &d.sub_label
        };
        let dot = if !d.sub_label.is_empty() {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        dot_group(dot, d.sub_color(), sub_display, &mut out);
    }
    out
}

// --- Pipe statusline: 720p │ en │ CC -------------------------------------
fn pipes(d: &IndicatorData) -> Vec<Span<'static>> {
    let sep = || Span::styled(" \u{2502} ", Style::default().fg(palette::OVERLAY));
    let mut out = vec![Span::styled(
        d.res_label.clone(),
        Style::default().fg(d.res_color()),
    )];
    if !d.audio_only {
        out.push(sep());
        out.push(Span::styled(
            d.audio_label.clone(),
            Style::default().fg(d.audio_color()),
        ));
        out.push(sep());
        let sub_display = if d.sub_label.is_empty() {
            "CC".to_string()
        } else {
            d.sub_label.clone()
        };
        out.push(Span::styled(
            sub_display,
            Style::default().fg(d.sub_color()),
        ));
    }
    out
}

// --- Labeled key·value: RES 720p  AUD en  SUB CC -------------------------
fn keyval_group(key: &str, value: &str, color: Color, out: &mut Vec<Span<'static>>) {
    out.push(Span::styled(
        format!("{key} "),
        Style::default().fg(palette::MUTED),
    ));
    out.push(Span::styled(value.to_string(), bold(color)));
}

fn keyvalue(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let res_key = if d.audio_only { "CODEC" } else { "RES" };
    keyval_group(res_key, &d.res_label, d.res_color(), &mut out);
    if !d.audio_only {
        out.push(Span::raw("  "));
        keyval_group("AUD", &d.audio_label, d.audio_color(), &mut out);
        out.push(Span::raw("  "));
        let sub_val = if d.sub_label.is_empty() {
            "\u{2014}".to_string()
        } else {
            d.sub_label.clone()
        };
        keyval_group("SUB", &sub_val, d.sub_color(), &mut out);
    }
    out
}

// --- Powerline segments (needs a patched font) ---------------------------
fn powerline(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut segs: Vec<(String, Color)> = vec![(d.res_label.clone(), d.res_color())];
    if !d.audio_only {
        segs.push((d.audio_label.clone(), d.audio_color()));
        let sub_display = if d.sub_label.is_empty() {
            "CC".to_string()
        } else {
            d.sub_label.clone()
        };
        segs.push((sub_display, d.sub_color()));
    }
    let mut out = Vec::new();
    for i in 0..segs.len() {
        let (ref label, color) = segs[i];
        out.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(color)
                .fg(palette::BASE)
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
