use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Size;
use ratatui::style::Color;
use ratatui_image::picker::Picker;
use ratatui_image::thread::ThreadProtocol;
use ratatui_image::{Resize, ResizeEncodeRender};

fn solid_image(width: u32, height: u32) -> image::DynamicImage {
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba([180, 40, 40, 255]),
    ))
}

fn projection() -> mbv_ui_model::playback::QueueCardProjection {
    mbv_ui_model::playback::QueueCardProjection {
        cache_key: Some("queue-art".to_owned()),
        plain_cache_key: None,
        images_enabled: true,
        visualizer: false,
        title_site: mbv_ui_model::playback::NowPlayingTitleSite::Header,
    }
}

fn is_painted(cell: &ratatui::buffer::Cell) -> bool {
    cell.symbol() != " " || cell.fg != Color::Reset || cell.bg != Color::Reset
}

/// A `ThreadProtocol` whose encode has already completed at `avail`: the
/// worker round trip is driven synchronously over the registered channel, so
/// the encoded area is known without a worker thread or timing.
fn encoded_protocol(avail: Size) -> (ThreadProtocol, Size) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut protocol = ThreadProtocol::new(
        tx,
        Some(Picker::halfblocks().new_resize_protocol(solid_image(20, 10))),
    );
    let held = protocol
        .size_for(Resize::Scale(Some(RENDER_FILTER)), avail)
        .expect("the source size is computable before the first encode");
    protocol.resize_encode(&Resize::Scale(Some(RENDER_FILTER)), held);
    let response = rx
        .recv()
        .expect("the encode request reaches the registered channel")
        .resize_encode()
        .expect("halfblocks encode succeeds");
    assert!(protocol.update_resized_protocol(response));
    (protocol, held)
}

fn draw_hold(area: Rect, last_card: (u16, u16), image: &mut ThreadProtocol) -> Buffer {
    let backend = TestBackend::new(area.width, area.height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            render_card_painting(
                f,
                area,
                false,
                &projection(),
                false,
                true,
                Some(image),
                last_card,
                50,
            );
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn protocol_ready(protocol: &ThreadProtocol, area: Rect) -> bool {
    protocol
        .size_for(
            Resize::Scale(Some(RENDER_FILTER)),
            Size {
                width: area.width,
                height: area.height,
            },
        )
        .is_some()
}

/// The settle hold paints the last encoded placement inside the moved slot
/// and leaves the protocol intact (no re-encode is taken), so a pinned panel
/// width change neither blanks the art nor re-transmits its payload
/// mid-animation.
#[test]
fn settle_hold_paints_the_last_encoded_placement_without_re_encoding() {
    let area = Rect::new(0, 0, 16, 12);
    let (mut protocol, held) = encoded_protocol(Size {
        width: 8,
        height: 6,
    });
    let buffer = draw_hold(area, (held.height, held.width), &mut protocol);
    let img_x = (area.width - held.width) / 2;
    for x in img_x..img_x + held.width {
        assert!(
            is_painted(&buffer[(x, 0)]),
            "the held placement paints at ({x}, 0)"
        );
    }
    assert!(
        !is_painted(&buffer[(0, 0)]),
        "cells left of the held placement stay unpainted"
    );
    assert!(
        !is_painted(&buffer[(area.width - 1, 0)]),
        "cells right of the held placement stay unpainted"
    );
    assert!(
        protocol_ready(&protocol, area),
        "the hold must not take the protocol for a re-encode"
    );
}

/// A held placement wider than the resized slot clips to the slot through
/// the direct protocol render instead of re-encoding at the smaller size.
#[test]
fn settle_hold_clips_to_the_slot_without_re_encoding() {
    let area = Rect::new(0, 0, 4, 12);
    let (mut protocol, held) = encoded_protocol(Size {
        width: 12,
        height: 6,
    });
    assert!(
        held.width > area.width,
        "case setup: the held art overflows"
    );
    let buffer = draw_hold(area, (held.height, held.width), &mut protocol);
    for x in 0..area.width {
        assert!(
            is_painted(&buffer[(x, 0)]),
            "the clipped placement paints at ({x}, 0)"
        );
    }
    assert!(
        protocol_ready(&protocol, area),
        "the clip must not take the protocol for a re-encode"
    );
}
