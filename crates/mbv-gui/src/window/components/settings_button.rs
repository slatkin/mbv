//! One Settings button (`replace-tauri-with-blitz` task 8.2, the port of
//! `SettingsButton.svelte`; add-gui-settings-screen design D13, D15): 36 px
//! high with a 6 px radius in four kinds — secondary (the raised fill and
//! the control border), primary (the accent fill and ink text), destructive
//! (the destructive outline), and ghost (muted text with no fill). A
//! disabled button paints as unavailable, so a press on it never runs its
//! handler (design D11's pointer contract). A button that activates a
//! content entry carries the entry's id and the content cursor's ring, so
//! the keyboard and the pointer rest on the same control.

use std::fmt::Write as _;

use dioxus_native::prelude::*;

/// The button's kind (`SettingsButton.svelte`'s `Variant`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsButtonVariant {
    /// The raised fill and the control border.
    #[default]
    Secondary,
    /// The accent fill and ink text.
    Primary,
    /// The destructive outline.
    Destructive,
    /// Muted text with no fill.
    Ghost,
}

impl SettingsButtonVariant {
    /// The wire name the `data-settings-button` attribute paints, as the
    /// Svelte variant's name.
    #[must_use]
    fn name(self) -> &'static str {
        match self {
            Self::Secondary => "secondary",
            Self::Primary => "primary",
            Self::Destructive => "destructive",
            Self::Ghost => "ghost",
        }
    }

    /// Each kind's fill, border, text colour, padding, and weight (the
    /// Svelte `VARIANTS` map).
    fn paint(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        Option<&'static str>,
    ) {
        match self {
            Self::Secondary => (
                "var(--canvas-library-raise)",
                "1px solid var(--canvas-library-control)",
                "var(--canvas-text)",
                "0 14px",
                None,
            ),
            Self::Primary => (
                "var(--canvas-accent)",
                "0",
                "var(--canvas-ink)",
                "0 16px",
                Some("600"),
            ),
            Self::Destructive => (
                "none",
                "1px solid var(--canvas-destructive-border)",
                "var(--canvas-destructive)",
                "0 14px",
                None,
            ),
            Self::Ghost => ("none", "0", "var(--canvas-muted)", "0 14px", None),
        }
    }
}

/// The props of the button (the `SettingsButton.svelte` props, in order).
#[derive(Props, Clone, PartialEq)]
pub struct SettingsButtonProps {
    /// The button's kind; secondary is the default.
    #[props(default)]
    pub variant: SettingsButtonVariant,
    /// Whether the action is currently unavailable.
    #[props(default)]
    pub disabled: bool,
    /// The content entry this button activates (`entryId` in the settings
    /// entries), painted for the section's press handler and the cursor fact.
    #[props(default)]
    pub entry: Option<String>,
    /// The button's accessible name, for a button whose label is a glyph.
    #[props(default)]
    pub label: Option<String>,
    /// Whether the content cursor rests on this button while the content
    /// pane holds the keyboard.
    #[props(default)]
    pub cursor: bool,
    /// A press on the button.
    #[props(default)]
    pub onpress: Option<Callback<()>>,
    /// The button's label.
    pub children: Element,
}

impl std::fmt::Debug for SettingsButtonProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsButtonProps")
            .field("variant", &self.variant)
            .field("disabled", &self.disabled)
            .field("entry", &self.entry)
            .field("label", &self.label)
            .field("cursor", &self.cursor)
            .finish_non_exhaustive()
    }
}

/// The one Settings button (the Svelte markup, character for character).
#[component]
pub fn SettingsButton(props: SettingsButtonProps) -> Element {
    let (background, border, color, padding, weight) = props.variant.paint();
    let mut style = format!(
        "height: 36px; padding: {padding}; background: {background}; border: {border}; border-radius: 6px; color: {}; font-size: 13px;",
        if props.disabled {
            "var(--canvas-ordinal)"
        } else {
            color
        },
    );
    if let Some(weight) = weight {
        let _ = write!(style, " font-weight: {weight};");
    }
    if props.disabled {
        style.push_str(" opacity: 0.7;");
    }
    if props.cursor {
        style.push_str(" outline: 2px solid var(--canvas-accent); outline-offset: -2px;");
    }
    let onpress = props.onpress;
    rsx! {
        button {
            "type": "button",
            class: "flex-none whitespace-nowrap",
            style: "{style}",
            "data-settings-button": props.variant.name(),
            "data-settings-entry": props.entry,
            "data-settings-cursor": props.cursor.then_some(""),
            "aria-label": props.label,
            disabled: props.disabled,
            onclick: move |_| {
                if let Some(onpress) = onpress {
                    onpress.call(());
                }
            },
            {props.children}
        }
    }
}
