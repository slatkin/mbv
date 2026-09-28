use mbv_images::RENDER_FILTER;
use mbv_theme as palette;
use mbv_ui_model::playback::QueueCardProjection;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;

/// The rectangle the queue card reserves for artwork: the last rendered
/// image/visualizer size, or the full reserved slot (capped like the artwork
/// height) before anything has rendered. Used by both the blank reservations
/// and the visualizer so `v` never moves the queue list.
/// The now-playing image's height cap: 12 rows under 40 rows of terminal
/// height, 16 under 50, 24 otherwise. Kept small enough that the queue
/// list below keeps the title separator and a few rows.
#[must_use]
pub fn queue_card_height_cap(height: u16) -> u16 {
    if height < 40 {
        12
    } else if height < 50 {
        16
    } else {
        24
    }
}

#[must_use]
pub fn queue_card_reserved_rect(
    last_card: (u16, u16),
    terminal_height: u16,
    area: Rect,
    left_align: bool,
) -> Rect {
    let (last_height, last_width) = last_card;
    let max_h = area.height.min(queue_card_height_cap(terminal_height));
    let height = if last_height == 0 {
        // The fallback slot is two terminal cells wide per row, matching
        // square artwork at the terminal's cell aspect. Constrain both axes:
        // reserving max_h without its matching width makes a narrow column
        // start tall and then shrink when the real image is measured.
        let width_limited = area.width.div_ceil(2);
        let height_limited = if max_h == area.height {
            // Keep the queue title separator and one queue row visible when
            // the card area is shorter than its normal image cap.
            max_h.saturating_sub(2)
        } else {
            max_h
        };
        height_limited.min(width_limited)
    } else {
        last_height
    };
    let width = if last_width == 0 {
        if left_align {
            height.saturating_mul(2).min(area.width)
        } else {
            area.width
        }
    } else {
        last_width
    };
    let x = if left_align {
        area.x
    } else {
        area.x + (area.width.saturating_sub(width)) / 2
    };
    Rect {
        x,
        y: area.y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

type CardImageProtocol = ratatui_image::thread::ThreadProtocol;

/// The queue visual slot's painter (task 3.4, design D9): a free function
/// over projected state with no `App` access. `image` is the shell-resolved
/// protocol handle for the slot's key (`None` while not ready); `loading`
/// reserves the loading rectangle for an in-flight fetch. Returns
/// `(rows_used, cols_used, image_loading)`.
///
/// # Panics
///
/// Panics only if the image protocol handle is unexpectedly absent after the
/// renderer has selected the image-painting path.
pub fn render_card_painting(
    f: &mut Frame,
    area: Rect,
    left_align: bool,
    projection: &QueueCardProjection,
    // `true` while a fetch for the slot's key is in flight: the painter
    // reserves the loading rectangle instead of leaving the slot blank.
    loading: bool,
    image: Option<&mut CardImageProtocol>,
    last_card: (u16, u16),
    terminal_height: u16,
) -> (u16, u16, bool) {
    // The visualizer never reaches this painter (the `App` adapter paints it
    // from the shell's sample window); degenerate input reserves geometry.
    if projection.visualizer || !projection.images_enabled {
        let rect = queue_card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    // The bundled placeholder slot caps its height at the same tier cap as
    // the real image (24 rows at full height), like the compact banner's
    // poster placeholder.
    let placeholder_slot = projection.cache_key.is_none();
    let cap = queue_card_height_cap(terminal_height);
    let max_h = if placeholder_slot {
        area.height.min(24)
    } else {
        area.height
    }
    .min(cap);
    let mut image = image;
    let actual_size = image.as_deref_mut().and_then(|state| {
        let avail = ratatui::layout::Size {
            width: area.width,
            height: max_h,
        };
        // `size_for` returns `None` while the resize+encode is still
        // in-flight on the worker thread (ThreadProtocol has taken its
        // inner protocol to send it off). Fall through to the
        // loading/placeholder path below for that frame; the next
        // frame after the response arrives will have a size again.
        state
            .size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            .map(|actual| (actual.height, actual.width, actual))
    });
    if let Some((height, width, _actual)) = actual_size {
        let img_x = if left_align {
            area.x
        } else {
            area.x + (area.width.saturating_sub(width)) / 2
        };
        let img_rect = Rect {
            x: img_x,
            y: area.y,
            width,
            height,
        };
        f.render_stateful_widget(
            ratatui_image::StatefulImage::default()
                .resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
            img_rect,
            image.expect("image handle present"),
        );
        return (height, width, false);
    }
    // No image loaded yet. If a fetch is in-flight and we have never rendered
    // a card before, reserve the full height cap so the queue panel doesn't
    // expand then collapse when the first image arrives; otherwise the last
    // painted geometry holds the slot steady.
    let (last_height, last_width) = last_card;
    let reservation = if last_height == 0 && loading {
        queue_card_reserved_rect(last_card, terminal_height, area, left_align)
    } else {
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: last_height,
        }
    };
    let placeholder_w = if last_width == 0 && loading {
        reservation.width
    } else {
        last_width
    };
    // The reserved area above was otherwise left visually blank while
    // loading -- paint a dim block over it instead, matching the
    // compact movie banner's own poster placeholder. Image aspect
    // ratios vary too widely here (backdrop, poster, album art,
    // thumbnail) to estimate a tighter width the way the banner does
    // for posters specifically, so this fills the full reserved area.
    if loading && reservation.height > 0 {
        f.render_widget(
            Block::default().style(Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            reservation,
        );
    }
    if placeholder_slot && reservation.height == 0 && last_width == 0 && !loading {
        // An empty queue with no previous artwork geometry still reserves its
        // fallback rectangle so toggling `v` never moves the queue list.
        let rect = queue_card_reserved_rect(last_card, terminal_height, area, left_align);
        return (rect.height, rect.width, false);
    }
    (reservation.height, placeholder_w, loading)
}
