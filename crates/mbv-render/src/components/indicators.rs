//! Status-indicator cluster treatments (resolution · audio · subtitles).
//!
//! The cluster's data — its style, the resolved facts, and the plain text
//! items — lives in `mbv_ui_model::indicators`; this module paints it. Six
//! interchangeable looks selected by config; the default is reverse-video
//! chips. Colors map onto the mbv palette: resolution = orange,
//! audio = blue (FOAM), subtitles = yellow (YELLOW) when on / dim when off.

use mbv_theme as palette;
use mbv_tui_support::ToRatatui;
use mbv_ui_model::indicators::{IndicatorData, IndicatorStyle};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

const ARROW: &str = "\u{E0B0}"; // powerline separator (needs a Nerd/Powerline font)

/// The render toolkit colour of the role the resolution label paints in.
fn res_color(d: &IndicatorData) -> Color {
    d.res_role().color().to_ratatui()
}

/// The render toolkit colour of the role the audio label paints in.
fn audio_color(d: &IndicatorData) -> Color {
    d.audio_role().color().to_ratatui()
}

/// The render toolkit colour of the role the subtitle label paints in.
fn sub_color(d: &IndicatorData) -> Color {
    d.sub_role().color().to_ratatui()
}

/// Build the fully-styled indicator spans for the chosen treatment.
/// Widths are self-describing (each span carries its own padding), so callers
/// can measure with `span.content.width()` and right/center-align as needed.
#[must_use]
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
            .fg(palette::Role::TextOnAccent.color().to_ratatui())
            .add_modifier(Modifier::BOLD),
    )
}

fn chips(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = vec![chip(&d.res_label, res_color(d))];
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        out.push(chip(&d.audio_label, audio_color(d)));
        out.push(Span::raw(" "));
        if d.flags.sub_on {
            out.push(chip(&d.sub_label, sub_color(d)));
        } else {
            // Off: hollow/dim — no fill, dim text.
            out.push(Span::styled(
                format!(" {} ", d.sub_label),
                bold(palette::Role::TextMuted.color().to_ratatui()),
            ));
        }
    }
    out
}

// --- Legacy brackets: [FHD] [en] [CC] -----------------------------------
fn bracket_group(label: &str, color: Color, out: &mut Vec<Span<'static>>) {
    let b = bold(palette::Role::TextStrong.color().to_ratatui());
    out.push(Span::styled("[", b));
    out.push(Span::styled(label.to_string(), bold(color)));
    out.push(Span::styled("]", b));
}

fn brackets(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    bracket_group(&d.res_label, res_color(d), &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        bracket_group(&d.audio_label, audio_color(d), &mut out);
        out.push(Span::raw(" "));
        bracket_group(&d.sub_label, sub_color(d), &mut out);
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
    outlined_group(&d.res_label, res_color(d), &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw(" "));
        outlined_group(&d.audio_label, audio_color(d), &mut out);
        out.push(Span::raw(" "));
        outlined_group(&d.sub_label, sub_color(d), &mut out);
    }
    out
}

// --- Status dots: ● label (uses filled glyph; opt-in) --------------------
fn dot_group(dot: &str, color: Color, label: &str, out: &mut Vec<Span<'static>>) {
    out.push(Span::styled(dot.to_string(), Style::default().fg(color)));
    out.push(Span::styled(
        format!(" {label}"),
        Style::default().fg(palette::Role::TextSecondary.color().to_ratatui()),
    ));
}

fn dots(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    dot_group("\u{25CF}", res_color(d), &d.res_label, &mut out);
    if !d.flags.audio_only {
        out.push(Span::raw("  "));
        dot_group("\u{25CF}", audio_color(d), &d.audio_label, &mut out);
        out.push(Span::raw("  "));
        let dot = if d.flags.sub_on {
            "\u{25CF}"
        } else {
            "\u{25CB}"
        };
        dot_group(dot, sub_color(d), &d.sub_label, &mut out);
    }
    out
}

// --- Pipe statusline: FHD │ en │ CC --------------------------------------
fn pipes(d: &IndicatorData) -> Vec<Span<'static>> {
    let sep = || {
        Span::styled(
            " \u{2502} ",
            Style::default().fg(palette::Role::TextMuted.color().to_ratatui()),
        )
    };
    let mut out = vec![Span::styled(
        d.res_label.clone(),
        Style::default().fg(res_color(d)),
    )];
    if !d.flags.audio_only {
        out.push(sep());
        out.push(Span::styled(
            d.audio_label.clone(),
            Style::default().fg(audio_color(d)),
        ));
        out.push(sep());
        out.push(Span::styled(
            d.sub_label.clone(),
            Style::default().fg(sub_color(d)),
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
    out.push(Span::styled(d.res_label.clone(), bold(res_color(d))));
    if !d.flags.audio_only {
        out.push(Span::styled(
            " ⧸ ",
            Style::default().fg(palette::Role::TextMuted.color().to_ratatui()),
        ));
        out.push(Span::styled(d.audio_label.clone(), bold(audio_color(d))));
        out.push(Span::styled(
            " ⧸ ",
            Style::default().fg(palette::Role::TextMuted.color().to_ratatui()),
        ));
        out.push(Span::styled(d.sub_label.clone(), bold(sub_color(d))));
    }
    out
}

// --- Powerline segments (needs a patched font) ---------------------------
fn powerline(d: &IndicatorData) -> Vec<Span<'static>> {
    let mut segs: Vec<(String, Color)> = vec![(d.res_label.clone(), res_color(d))];
    if !d.flags.audio_only {
        segs.push((d.audio_label.clone(), audio_color(d)));
        segs.push((d.sub_label.clone(), sub_color(d)));
    }
    let mut out = Vec::new();
    for i in 0..segs.len() {
        let (ref label, color) = segs[i];
        out.push(Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(color)
                .fg(palette::Role::TextOnAccent.color().to_ratatui())
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
