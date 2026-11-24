//! The ellipsis in mbv (D12 of `replace-tauri-with-blitz`): the `Truncate`
//! component and the pure cut function [`fit_text`] behind it. Blitz has no
//! `text-overflow: ellipsis` (gap G1), so mbv measures the text itself over
//! one process-wide parley context holding the seven bundled faces, and writes
//! a cut text that ends in `…` when the full text does not fit.
//!
//! A box mounts unmeasured: its first measure can read the layout from before
//! its insertion, a zero width, and then keeps its whole text. The mount is a
//! tree change, so the frame that resolves it reports the change and bumps the
//! [`LayoutEpoch`] once — after the resolve — which re-runs this box's measure
//! against the layout that holds it. No request is needed: the frame that
//! resolves any change re-measures every box.

use std::borrow::Cow;
use std::sync::{LazyLock, Mutex};

use blitz_dom::decode_font_bytes;
use dioxus_native::prelude::*;
use parley::style::{FontFamily, FontWeight, StyleProperty};
use unicode_segmentation::UnicodeSegmentation;

use crate::window::layout::LayoutEpoch;
use crate::window::net::font_bytes;

/// The text style a call site measures (D12): the family, size, weight, and
/// letter spacing the class string names. `TextStyle::sans(13.0, 500.0)` is
/// the port of a 13 px medium body text box's classes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// The face family the class string names.
    family: Family,
    /// The font size, in CSS pixels.
    size: f32,
    /// The CSS font weight: 400, 500, or 600 on either face, or 700 on the mono
    /// face.
    weight: f32,
    /// The letter spacing, in em, as a Tailwind `tracking-*` value names it.
    letter_spacing: f32,
}

/// The bundled face families (D11): the sans body face and the mono face.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Family {
    /// `IBM Plex Sans`, the body face.
    Sans,
    /// `JetBrains Mono`, the mono face.
    Mono,
}

impl Family {
    /// The family name the fonts registered under (D11).
    fn name(self) -> &'static str {
        match self {
            Family::Sans => "IBM Plex Sans",
            Family::Mono => "JetBrains Mono",
        }
    }
}

impl TextStyle {
    /// The sans face at `size` CSS pixels and CSS weight `weight`.
    #[must_use]
    pub fn sans(size: f32, weight: f32) -> Self {
        Self {
            family: Family::Sans,
            size,
            weight,
            letter_spacing: 0.0,
        }
    }

    /// The mono face at `size` CSS pixels and CSS weight `weight`.
    #[must_use]
    pub fn mono(size: f32, weight: f32) -> Self {
        Self {
            family: Family::Mono,
            size,
            weight,
            letter_spacing: 0.0,
        }
    }

    /// Letter spacing in em, for a class string that carries a `tracking-*`
    /// value.
    #[must_use]
    pub fn tracking(mut self, em: f32) -> Self {
        self.letter_spacing = em;
        self
    }
}

/// The process measure (D12): one parley font context with the six bundled
/// faces, registered once, and one layout context. A `Mutex`, because the
/// measure runs on the core thread's render ticks and in tests.
struct Measure {
    fonts: parley::FontContext,
    layout: parley::LayoutContext,
}

static MEASURE: LazyLock<Mutex<Measure>> = LazyLock::new(|| Mutex::new(Measure::new()));

impl Measure {
    /// A context with the seven bundled faces (D11) decoded from their WOFF2
    /// bytes.
    fn new() -> Self {
        let mut fonts = parley::FontContext::default();
        for face in FACE_FILES {
            let bytes = font_bytes(face).expect("every listed face is bundled");
            fonts.collection.register_fonts(
                parley::fontique::Blob::from(decode_font_bytes(bytes).into_owned()),
                None,
            );
        }
        Self {
            fonts,
            layout: parley::LayoutContext::new(),
        }
    }

    /// The width of `text` at `style`, in CSS pixels, on one line.
    fn width(&mut self, text: &str, style: &TextStyle) -> f32 {
        let mut builder = self
            .layout
            .ranged_builder(&mut self.fonts, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            Cow::Borrowed(style.family.name()),
        )));
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight)));
        builder.push_default(StyleProperty::LetterSpacing(style.letter_spacing));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.width()
    }
}

/// The seven bundled face files (D11), as `net.rs` serves them.
const FACE_FILES: &[&str] = &[
    "ibm-plex-sans-latin-400-normal.woff2",
    "ibm-plex-sans-latin-500-normal.woff2",
    "ibm-plex-sans-latin-600-normal.woff2",
    "jetbrains-mono-latin-400-normal.woff2",
    "jetbrains-mono-latin-500-normal.woff2",
    "jetbrains-mono-latin-600-normal.woff2",
    "jetbrains-mono-latin-700-normal.woff2",
];

/// The width of `text` at `style`, in CSS pixels, on one line: the measure
/// the fit and the tab bar's overflow window share.
///
/// # Panics
/// Panics when the process measure is poisoned — only possible if a caller
/// panicked while holding it.
#[must_use]
pub fn text_width(text: &str, style: &TextStyle) -> f32 {
    MEASURE
        .lock()
        .expect("the measure is not poisoned")
        .width(text, style)
}

/// The longest text that fits `width` CSS pixels at `style` (D12): the text
/// unchanged when it fits, else the longest grapheme-cluster prefix whose
/// width plus the ellipsis width fits, ending in `…` (U+2026). A binary
/// search over the grapheme boundaries finds the prefix. The box width is
/// `f64` — the client rect's unit — widened from the `f32` measures it
/// compares against.
///
/// A width of zero or less is a box the layout has not measured yet — a
/// document that never resolved, or a box a layout pass collapsed. Such a box
/// keeps its whole text, and its `truncate` class clips it: measuring against
/// zero would paint the ellipsis alone.
///
/// # Panics
/// Panics when the process measure is poisoned — only possible if a caller
/// panicked while holding it.
#[must_use]
pub fn fit_text(text: &str, style: &TextStyle, width: f64) -> String {
    const ELLIPSIS: &str = "\u{2026}";
    if width <= 0.0 {
        return text.to_owned();
    }
    let mut measure = MEASURE.lock().expect("the measure is not poisoned");
    if f64::from(measure.width(text, style)) <= width {
        return text.to_owned();
    }
    let ellipsis_width = f64::from(measure.width(ELLIPSIS, style));
    let ends: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(index, cluster)| index + cluster.len())
        .collect();
    let mut fits =
        |end: usize| f64::from(measure.width(&text[..end], style)) + ellipsis_width <= width;
    let mut low = 0;
    let mut high = ends.len();
    let mut best = None;
    while low < high {
        let mid = low + (high - low) / 2;
        if fits(ends[mid]) {
            best = Some(mid);
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    match best {
        Some(index) => format!("{}\u{2026}", &text[..ends[index]]),
        // Even the ellipsis does not fit; the box clips it.
        None => ELLIPSIS.to_owned(),
    }
}

/// The `Truncate` component (D12): the port of a text box whose the former client markup
/// carries the `truncate` class. It renders the box with the call site's
/// classes plus `truncate` — so a measure that is one frame late still clips
/// (`overflow: hidden`, `white-space: nowrap`) — and its current display
/// text, which the measure cuts when the full text does not fit.
#[derive(Props, Clone, PartialEq)]
pub struct TruncateProps {
    /// The full text.
    pub text: String,
    /// the former client class string, without `truncate` — the component appends it.
    #[props(default)]
    pub class: String,
    /// the former client element's inline `style:` string — the attributes the
    /// markup carried beside the `truncate` class (the colour and font size
    /// the text paints at, which the [`TextStyle`] only mirrors for the
    /// measure).
    #[props(default)]
    pub css: String,
    /// The further attributes the call site's box carries beside its classes
    /// and style — the `data-*` paint hooks the former markup set on the same
    /// box.
    #[props(default)]
    pub attrs: Vec<Attribute>,
    /// The style the class string names, for the measure.
    pub style: TextStyle,
}

impl std::fmt::Debug for TruncateProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TruncateProps")
            .field("text", &self.text)
            .field("class", &self.class)
            .field("css", &self.css)
            .field("attrs", &self.attrs)
            .field("style", &self.style)
            .finish()
    }
}

#[component]
pub fn Truncate(props: TruncateProps) -> Element {
    let mut display = use_signal(|| props.text.clone());
    let mut mounted: Signal<Option<Event<MountedData>>> = use_signal(|| None);
    // A missing epoch provider never re-measures on a layout change: the
    // full text still paints, and the box's `truncate` class still clips it.
    let epoch = try_consume_context::<LayoutEpoch>().map(|epoch| epoch.0);
    use_effect(use_reactive(
        (&props.text, &props.style),
        move |(text, style)| {
            let event = mounted();
            // Reading the epoch subscribes the re-measure to every layout change
            // (D12): a resize, a divider drag, or a panel state change re-measures
            // the box.
            let _epoch = epoch.as_ref().map(Signal::cloned);
            spawn(async move {
                let Some(event) = event else {
                    return;
                };
                let Ok(rect) = event.get_client_rect().await else {
                    return;
                };
                // The client rect is already CSS pixels: it reads the taffy
                // layout, whose viewport is the logical (physical / scale) size.
                let width = rect.width();
                let cut = fit_text(&text, &style, width);
                // A peek, so the write below cannot re-trigger this effect.
                if cut != display.peek().clone() {
                    display.set(cut);
                }
            });
        },
    ));

    let class = if props.class.is_empty() {
        "truncate".to_owned()
    } else {
        format!("{} truncate", props.class)
    };
    rsx! {
        div {
            class: "{class}",
            style: "{props.css}",
            onmounted: move |event| mounted.set(Some(event)),
            ..props.attrs,
            "{display}"
        }
    }
}

#[cfg(test)]
mod tests;
