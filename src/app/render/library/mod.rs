mod album;
mod season_grid;
mod table;

use super::super::layout::LayoutLibrary;
use super::super::palette;
use super::super::App;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

impl App {
    // crumb_area: when Some, the playback panel's bottom line is available as a
    // shared breadcrumb row — skip the top border and write crumb text there instead.
    // When None, own the top border (showing it only when there's something to display).
    pub(super) fn render_library(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        crumb_area: Option<Rect>,
        layout: &mut LayoutLibrary,
    ) {
        if lib_idx >= self.libs.len() {
            return;
        }
        // Self-heal, mirroring render_power_list's own inline call: the
        // only other place that triggers a library's load is
        // set_tab()/activate_library_position_scope(), which never runs
        // for the tab restored directly as `tab_idx` at startup (#248) --
        // without this, that tab's nav_stack stays empty forever and
        // shows "Loading..." with no fetch ever spawned.
        self.ensure_lib_loaded_for(lib_idx);
        let panel_visible = crumb_area.is_some();
        let is_loading = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.loading)
            .unwrap_or(true);
        if is_loading && self.libs[lib_idx].search.is_none() {
            if panel_visible {
                let mid = area.y + area.height / 2;
                f.render_widget(
                    Paragraph::new("Loading...")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(palette::MUTED)),
                    Rect {
                        y: mid,
                        height: 1,
                        ..area
                    },
                );
            } else {
                let block = Block::default()
                    .borders(Borders::TOP)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(palette::IRIS));
                let inner = block.inner(area);
                f.render_widget(block, area);
                let mid = inner.y + inner.height / 2;
                f.render_widget(
                    Paragraph::new("Loading...")
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(palette::MUTED)),
                    Rect {
                        y: mid,
                        height: 1,
                        ..inner
                    },
                );
            }
            return;
        }

        let lib = &self.libs[lib_idx];
        let skip = if lib
            .nav_stack
            .first()
            .map(|l| l.title == lib.library.name)
            .unwrap_or(false)
        {
            1
        } else {
            0
        };
        let mut crumb_names: Vec<(String, usize)> = vec![(lib.library.name.clone(), 1)];
        for (i, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
            crumb_names.push((lvl.title.clone(), i + 1));
        }

        let sep = "/";
        let is_deep = crumb_names.len() > 1;
        let has_search = self.libs[lib_idx].search.is_some();
        let show_border = !panel_visible && (is_deep || has_search);

        let crumb_row = crumb_area.map(|a| a.y).unwrap_or(area.y);
        let mut x = area.x + 2;

        let crumb_parent_style = Style::default().fg(palette::MUTED);
        let crumb_current_style = Style::default()
            .fg(palette::YELLOW)
            .add_modifier(Modifier::BOLD);
        let mut crumb_spans: Vec<Span<'static>> = Vec::new();
        let mut new_breadcrumbs: Vec<(u16, u16, u16, usize)> = Vec::new();
        for (ci, (name, target_depth)) in crumb_names.iter().enumerate() {
            let is_last = ci + 1 == crumb_names.len();
            // Parent levels show [N] instead of their full name to save space.
            let display: String = if is_last {
                name.clone()
            } else {
                format!("[{}]", ci + 1)
            };
            let w = display.chars().count() as u16;
            new_breadcrumbs.push((x, x + w, crumb_row, *target_depth));
            let style = if is_last {
                crumb_current_style
            } else {
                crumb_parent_style
            };
            crumb_spans.push(Span::styled(display, style));
            x += w;
            if !is_last {
                crumb_spans.push(Span::styled(sep, Style::default().fg(palette::IRIS)));
                x += sep.len() as u16;
            }
        }
        layout.breadcrumbs = if is_deep { new_breadcrumbs } else { Vec::new() };

        // Build the search/crumb label and render it — either onto crumb_area
        // (panel visible) or as a block title on the top border (panel hidden).
        let search_label: Option<Line<'static>> =
            if let Some(s) = self.libs[lib_idx].search.as_ref() {
                let label = if s.loading {
                    format!(
                        "Search {} (loading…): {}█",
                        self.libs[lib_idx].library.name, s.query
                    )
                } else {
                    format!("Search {}: {}█", self.libs[lib_idx].library.name, s.query)
                };
                let border_style = Style::default().fg(palette::IRIS);
                let text_style = Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD);
                Some(Line::from(vec![
                    Span::styled("─", border_style),
                    Span::raw(" "),
                    Span::styled(label, text_style),
                    Span::raw(" "),
                ]))
            } else if is_deep {
                let mut spans = crumb_spans;
                spans.insert(0, Span::raw(" "));
                spans.push(Span::raw(" "));
                Some(Line::from(spans))
            } else {
                None
            };

        let inner = if panel_visible {
            if let (Some(label), Some(ca)) = (search_label, crumb_area) {
                f.render_widget(Paragraph::new(label), ca);
            }
            area
        } else if show_border {
            let mut block = Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS));
            if let Some(label) = search_label {
                block = block.title(label);
            }
            let inner = block.inner(area);
            f.render_widget(block, area);
            inner
        } else {
            area
        };

        if self.is_album_level(lib_idx) && self.libs[lib_idx].search.is_none() {
            self.render_album_view(f, inner, lib_idx);
        } else {
            self.render_library_table(f, inner, lib_idx, layout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::LibraryTab;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    /// Issue #248: a library tab that's active from a *restored* `tab_idx`
    /// at startup never has its content loaded, because `build()` sets
    /// `tab_idx` as a raw struct field (not via `set_tab()`, which is the
    /// only thing that calls `ensure_lib_loaded_for` for the plain library
    /// path). Unlike Power View's `render_power_list` (which calls
    /// `ensure_lib_loaded_for` on every frame as a self-heal), plain
    /// `render_library` had no equivalent -- so a library with an empty
    /// `nav_stack` rendered "Loading..." forever with no fetch ever
    /// spawned. This asserts rendering alone -- with no prior `set_tab()`
    /// call -- is enough to kick off the load, exactly like Power View.
    #[test]
    fn render_library_triggers_load_for_never_visited_library_with_no_prior_set_tab() {
        let mut app = make_app_stub();
        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.collection_type = "movies".into();
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutLibrary::default();
        term.draw(|f| {
            app.render_library(f, Rect::new(0, 0, 60, 20), 0, None, &mut layout);
        })
        .unwrap();

        assert_eq!(
            app.libs[0].nav_stack.len(),
            1,
            "render_library must trigger ensure_lib_loaded_for for a never-visited \
             library, the same way render_power_list does, so a library restored as \
             the startup tab_idx isn't stuck on \"Loading...\" forever"
        );
        assert!(app.libs[0].nav_stack[0].loading);
    }
}
