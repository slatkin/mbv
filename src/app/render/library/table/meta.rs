use super::super::super::super::palette;
use super::super::super::super::App;
use super::LibraryTableContext;
use mbv_core::api::{MediaItem, TICKS_PER_SECOND};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// Formats runtime as "Xh0Ym" or "Ym" for the library-row meta line, or
/// `None` for zero/unknown runtime (in which case callers omit it from the
/// meta parts entirely — unlike `ui_util::fmt_duration_approx`, which has a
/// distinct "<1m" case for sub-minute runtimes that isn't wanted here).
fn library_duration_part(runtime_ticks: i64) -> Option<String> {
    let dur_s = runtime_ticks / TICKS_PER_SECOND;
    if dur_s <= 0 {
        return None;
    }
    let h = dur_s / 3600;
    let m = (dur_s % 3600) / 60;
    Some(if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m")
    })
}

pub(super) fn library_folder_count(item: &MediaItem) -> Option<u32> {
    if item.is_folder && item.item_type == "Folder" && item.total_count > 0 {
        Some(item.total_count)
    } else {
        None
    }
}

pub(super) fn library_title_line(item: &MediaItem) -> String {
    match item.item_type.as_str() {
        "Episode" => {
            let n = item.index_number;
            if n > 0 {
                format!("{}. {}", n, item.name)
            } else {
                item.name.clone()
            }
        }
        "Series" => {
            if item.total_count > 0 {
                format!(
                    "{} ({}/{})",
                    item.name, item.unplayed_item_count, item.total_count
                )
            } else if item.unplayed_item_count > 0 {
                format!("{} ({})", item.name, item.unplayed_item_count)
            } else {
                item.name.clone()
            }
        }
        _ => item.name.clone(),
    }
}

pub(super) fn library_meta_line(
    app: &App,
    item: &MediaItem,
    is_audio: bool,
    is_album_folder: bool,
    is_episode_like: bool,
    ctx: &LibraryTableContext,
) -> Line<'static> {
    let meta_line: Line = if is_episode_like && item.item_type != "Episode" {
        episode_meta_line(item, ctx.is_feed_lib)
    } else {
        match item.item_type.as_str() {
            "Series" => {
                let year_str = if item.production_year > 0
                    && item.end_year > 0
                    && item.end_year != item.production_year
                {
                    format!("{} – {}", item.production_year, item.end_year)
                } else if item.production_year > 0 && item.end_year == 0 {
                    format!("{} –", item.production_year)
                } else if item.production_year > 0 {
                    format!("{}", item.production_year)
                } else {
                    String::new()
                };
                Line::from(Span::styled(year_str, Style::default().fg(palette::SUBTLE)))
            }
            "Season" => {
                let mut parts: Vec<String> = Vec::new();
                if item.total_count > 0 {
                    parts.push(format!("{} eps", item.total_count));
                }
                if item.production_year > 0 {
                    parts.push(format!("{}", item.production_year));
                }
                Line::from(Span::styled(
                    parts.join(" · "),
                    Style::default().fg(palette::SUBTLE),
                ))
            }
            "Episode" => episode_meta_line(item, ctx.is_feed_lib),
            _ if library_is_generic_folder(item) => {
                if is_album_folder {
                    let mut parts: Vec<String> = Vec::new();
                    let year = if item.production_year > 0 {
                        item.production_year
                    } else {
                        app.album_year_cache.get(&item.id).copied().unwrap_or(0)
                    };
                    if year > 0 {
                        parts.push(format!("{}", year));
                    }
                    if item.total_count > 0 {
                        parts.push(format!("{} tracks", item.total_count));
                    }
                    Line::from(Span::styled(
                        parts.join("  "),
                        Style::default().fg(palette::SUBTLE),
                    ))
                } else if item.total_count > 0 {
                    Line::from(Span::styled(
                        format!("{} items", item.total_count),
                        Style::default().fg(palette::SUBTLE),
                    ))
                } else {
                    Line::from(vec![])
                }
            }
            _ => default_meta_line(item, is_audio),
        }
    };

    if item.is_folder && item.item_type != "Series" && item.item_type != "Season" {
        return meta_line;
    }

    let type_str = if matches!(item.item_type.as_str(), "Movie" | "Series") {
        if !item.genre.is_empty() {
            item.genre.clone()
        } else {
            String::new()
        }
    } else if item.item_type == "Episode" || is_episode_like {
        String::new()
    } else if !item.item_type.is_empty() {
        item.item_type.clone()
    } else {
        "—".to_string()
    };
    if type_str.is_empty() {
        meta_line
    } else {
        let mut spans = vec![Span::styled(
            format!("{}  ", type_str),
            Style::default().fg(palette::SUBTLE),
        )];
        spans.extend(meta_line.spans);
        Line::from(spans)
    }
}

fn episode_meta_line(item: &MediaItem, is_feed_lib: bool) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    if item.played {
        spans.push(Span::styled("✓ ", Style::default().fg(palette::PINE)));
    }
    let mut parts: Vec<String> = Vec::new();
    if !item.premiere_date.is_empty() {
        parts.push(item.premiere_date.clone());
    }
    if is_feed_lib && !item.date_added.is_empty() {
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let formatted = item
            .date_added
            .splitn(3, '-')
            .collect::<Vec<_>>()
            .as_slice()
            .windows(3)
            .next()
            .and_then(|p| {
                let y = p[0];
                let d: u32 = p[2].parse().ok()?;
                let m: usize = p[1].parse::<usize>().ok()?.checked_sub(1)?;
                Some(format!("Added {} {}, {}", d, MONTHS.get(m)?, y))
            })
            .unwrap_or_else(|| item.date_added.clone());
        parts.push(formatted);
    }
    if let Some(dur) = library_duration_part(item.runtime_ticks) {
        parts.push(dur);
    }
    if !parts.is_empty() {
        spans.push(Span::styled(
            parts.join("  "),
            Style::default().fg(palette::SUBTLE),
        ));
    }
    if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
        spans.push(Span::styled(
            format!("  {pct}%"),
            Style::default().fg(palette::YELLOW),
        ));
    }
    Line::from(spans)
}

fn default_meta_line(item: &MediaItem, is_audio: bool) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    if !is_audio && item.played {
        spans.push(Span::styled("✓ ", Style::default().fg(palette::PINE)));
    }
    let mut parts: Vec<String> = Vec::new();
    if item.production_year > 0 {
        parts.push(format!("{}", item.production_year));
    }
    if let Some(dur) = library_duration_part(item.runtime_ticks) {
        parts.push(dur);
    }
    if is_audio && !item.container.is_empty() {
        parts.push(item.container.to_uppercase());
    }
    if !parts.is_empty() {
        spans.push(Span::styled(
            parts.join("  "),
            Style::default().fg(palette::SUBTLE),
        ));
    }
    if !is_audio && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
        spans.push(Span::styled(
            format!("  {pct}%"),
            Style::default().fg(palette::YELLOW),
        ));
    }
    Line::from(spans)
}

pub(super) fn library_is_audio(item: &MediaItem) -> bool {
    item.media_type == "Audio" || item.item_type == "Audio"
}

pub(super) fn library_is_episode_like(item: &MediaItem, is_feed_lib: bool) -> bool {
    item.item_type == "Episode" || (is_feed_lib && item.item_type == "Video")
}

pub(super) fn library_is_generic_folder(item: &MediaItem) -> bool {
    item.is_folder && item.item_type != "Series" && item.item_type != "Season"
}
