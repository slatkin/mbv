/// Per-section configured bindings: router-scope overrides and
/// prefix-namespace assignments, each mapping a declared action id to its
/// configured chord.
use super::chord::Chord;
use super::registry::{action_by_id, KeySection, KeybindAction, KEYBIND_ACTIONS, RESERVED_CHORDS};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SectionBindings {
    pub router: Vec<(&'static str, Chord)>,
    pub prefix: Vec<(&'static str, Chord)>,
}

/// The compiled keybind configuration: the optional prefix chord plus
/// per-section router overrides and prefix-namespace assignments. Actions
/// without an override resolve to their declared defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keybinds {
    pub prefix: Option<Chord>,
    pub sections: Vec<(KeySection, SectionBindings)>,
}

impl Keybinds {
    /// The all-defaults configuration: no prefix, every action on its
    /// declared chords. Compiled from `KEYBIND_ACTIONS` through the
    /// resolution helpers below.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// The configured router-scope chord for an action id, if any.
    pub fn router_override(&self, id: &str) -> Option<Chord> {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.router.iter())
            .find(|(action_id, _)| *action_id == id)
            .map(|(_, chord)| *chord)
    }

    /// The configured prefix-namespace chord for an action id, if any.
    pub fn prefix_assignment(&self, id: &str) -> Option<Chord> {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.prefix.iter())
            .find(|(action_id, _)| *action_id == id)
            .map(|(_, chord)| *chord)
    }

    /// The chords that fire an action under this configuration: its single
    /// configured chord, or all of its declared default chords.
    pub fn router_chords(&self, action: &KeybindAction) -> Vec<Chord> {
        match self.router_override(action.id) {
            Some(chord) => vec![chord],
            None => action.parsed_default_chords(),
        }
    }

    /// The first chord that fires an action under this configuration.
    pub fn router_chord(&self, action: &KeybindAction) -> Chord {
        self.router_chords(action)
            .into_iter()
            .next()
            .expect("every action resolves to at least one chord")
    }

    /// Every configured prefix-namespace assignment as (action, configured
    /// chord), for the armed-dispatch lookup (change
    /// `add-configurable-keybinds`, design D6). Ids are validated at load.
    pub fn prefix_assignments(&self) -> impl Iterator<Item = (&'static KeybindAction, Chord)> + '_ {
        self.sections
            .iter()
            .flat_map(|(_, bindings)| bindings.prefix.iter())
            .filter_map(|(id, chord)| Some((action_by_id(id)?, *chord)))
    }

    /// The number of actions whose router-scope binding deviates from the
    /// declared default: the settings main list's Keys-row summary count
    /// (design D7). Counted as distinct action ids — an action bound by
    /// more than one router entry still counts once — and a configured
    /// chord equal to a declared default is not a deviation.
    pub fn override_count(&self) -> usize {
        let mut overridden: Vec<&'static str> = self
            .sections
            .iter()
            .flat_map(|(_, bindings)| bindings.router.iter())
            .filter(|(id, chord)| {
                action_by_id(id)
                    .is_some_and(|action| !action.parsed_default_chords().contains(chord))
            })
            .map(|(id, _)| *id)
            .collect();
        overridden.sort_unstable();
        overridden.dedup();
        overridden.len()
    }
}

/// The `[keys]` configuration as read from the file, before compilation:
/// section-outer shape with the prefix chord and per-section router /
/// prefix-namespace chord assignments (design D3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawKeybinds {
    pub prefix: Option<String>,
    pub sections: Vec<(String, RawSection)>,
}

/// One section's entries as read from the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSection {
    pub router: Vec<(String, String)>,
    pub prefix: Vec<(String, String)>,
}

/// A load-time rejection, naming the offending entry (both entries for the
/// collision classes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeybindsError {
    ReservedChord {
        chord: String,
        entry: String,
    },
    UnparseableChord {
        chord: String,
        entry: String,
        reason: String,
    },
    UnknownAction {
        action: String,
        section: String,
    },
    UnknownSection {
        section: String,
    },
    DuplicateSection {
        first: String,
        second: String,
    },
    SectionMismatch {
        action: String,
        section: String,
        declared: KeySection,
    },
    NotPrefixAddressable {
        action: String,
        section: String,
    },
    PrefixCollision {
        chord: String,
        entry: String,
    },
    RouterCollision {
        chord: String,
        first: String,
        second: String,
    },
    PrefixNamespaceCollision {
        chord: String,
        first: String,
        second: String,
    },
}

impl std::fmt::Display for KeybindsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReservedChord { chord, entry } => write!(
                f,
                "keys: chord `{chord}` in `{entry}` is reserved and cannot be configured"
            ),
            Self::UnparseableChord {
                chord,
                entry,
                reason,
            } => write!(f, "keys: `{entry}` has unparseable chord `{chord}`: {reason}"),
            Self::UnknownAction { action, section } => write!(
                f,
                "keys: unknown action `{action}` in section `keys.{section}`"
            ),
            Self::UnknownSection { section } => {
                write!(f, "keys: unknown section `keys.{section}`")
            }
            Self::DuplicateSection { first, second } => write!(
                f,
                "keys: sections `keys.{first}` and `keys.{second}` name the same section under different spellings"
            ),
            Self::SectionMismatch {
                action,
                section,
                declared,
            } => write!(
                f,
                "keys: action `{action}` is declared in section `{}` but configured under `keys.{section}`",
                declared.name()
            ),
            Self::NotPrefixAddressable { action, section } => write!(
                f,
                "keys: action `{action}` is not prefix-addressable (`keys.{section}.prefix.{action}`)"
            ),
            Self::PrefixCollision { chord, entry } => write!(
                f,
                "keys: prefix chord `{chord}` collides with binding `{entry}`"
            ),
            Self::RouterCollision {
                chord,
                first,
                second,
            } => write!(
                f,
                "keys: actions `{first}` and `{second}` share router-scope chord `{chord}`"
            ),
            Self::PrefixNamespaceCollision {
                chord,
                first,
                second,
            } => write!(
                f,
                "keys: actions `{first}` and `{second}` share prefix-namespace chord `{chord}`"
            ),
        }
    }
}

impl std::error::Error for KeybindsError {}

/// Compile a raw `[keys]` configuration into validated `Keybinds`, rejecting
/// every malformed or colliding entry at load (design D5).
pub fn load(raw: &RawKeybinds) -> Result<Keybinds, KeybindsError> {
    let reserved: Vec<Chord> = RESERVED_CHORDS
        .iter()
        .map(|s| Chord::parse(s).expect("reserved chord must parse"))
        .collect();

    let prefix = raw
        .prefix
        .as_ref()
        .map(|s| parse_configured(s, "keys.prefix", &reserved))
        .transpose()?;

    let mut sections = Vec::new();
    // `KeySection::from_name` matches case-insensitively, but the save loop
    // keys the compiled tables by lowercase section name — two raw spellings
    // of one section would silently collapse to the later spelling on save
    // while the reader took the first. Reject them at load.
    let mut seen_sections: Vec<&str> = Vec::new();
    let mut configured_router: Vec<(&'static KeybindAction, Chord)> = Vec::new();
    let mut configured_prefix_ns: Vec<(&'static KeybindAction, Chord)> = Vec::new();

    for (section_name, raw_section) in &raw.sections {
        if let Some(first) = seen_sections
            .iter()
            .find(|seen| seen.eq_ignore_ascii_case(section_name))
        {
            return Err(KeybindsError::DuplicateSection {
                first: (*first).to_string(),
                second: section_name.clone(),
            });
        }
        seen_sections.push(section_name);
        let section =
            KeySection::from_name(section_name).ok_or_else(|| KeybindsError::UnknownSection {
                section: section_name.clone(),
            })?;
        let mut bindings = SectionBindings::default();
        for (id, chord) in &raw_section.router {
            let action = lookup_in_section(id, section_name)?;
            let entry = format!("keys.{section_name}.{id}");
            let parsed = parse_configured(chord, &entry, &reserved)?;
            configured_router.push((action, parsed));
            bindings.router.push((action.id, parsed));
        }
        for (id, chord) in &raw_section.prefix {
            let action = lookup_in_section(id, section_name)?;
            if !action.prefix_addressable {
                return Err(KeybindsError::NotPrefixAddressable {
                    action: id.clone(),
                    section: section_name.clone(),
                });
            }
            let entry = format!("keys.{section_name}.prefix.{id}");
            let parsed = parse_configured(chord, &entry, &reserved)?;
            configured_prefix_ns.push((action, parsed));
            bindings.prefix.push((action.id, parsed));
        }
        sections.push((section, bindings));
    }

    let keybinds = Keybinds { prefix, sections };

    // The prefix chord must not collide with any other configured or declared
    // binding (router scope or prefix namespace).
    if let Some(prefix) = keybinds.prefix {
        for (action, chord) in &configured_router {
            if *chord == prefix {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.{}", action.section.name(), action.id),
                });
            }
        }
        for (action, chord) in &configured_prefix_ns {
            if *chord == prefix {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.prefix.{}", action.section.name(), action.id),
                });
            }
        }
        for action in KEYBIND_ACTIONS {
            if configured_router
                .iter()
                .any(|(configured, _)| configured.id == action.id)
            {
                continue;
            }
            if action.parsed_default_chords().contains(&prefix) {
                return Err(KeybindsError::PrefixCollision {
                    chord: prefix.to_string(),
                    entry: format!("keys.{}.{}", action.section.name(), action.id),
                });
            }
        }
    }

    // Router-scope: a configured chord must not equal any other action's
    // effective chord (configured or declared default). Pure default-vs-
    // default pairs are exempt — the declaration test pins that no two
    // actions share a default chord.
    for (action, chord) in &configured_router {
        for other in KEYBIND_ACTIONS {
            if other.id == action.id {
                continue;
            }
            let other_effective = match configured_router
                .iter()
                .find(|(configured, _)| configured.id == other.id)
            {
                Some((_, configured)) => vec![*configured],
                None => other.parsed_default_chords(),
            };
            if other_effective.contains(chord) {
                return Err(KeybindsError::RouterCollision {
                    chord: chord.to_string(),
                    first: action.id.to_string(),
                    second: other.id.to_string(),
                });
            }
        }
    }

    // Prefix namespace: two configured assignments must not share a chord.
    for (index, (first, chord)) in configured_prefix_ns.iter().enumerate() {
        for (second, other_chord) in configured_prefix_ns.iter().skip(index + 1) {
            if first.id != second.id && chord == other_chord {
                return Err(KeybindsError::PrefixNamespaceCollision {
                    chord: chord.to_string(),
                    first: first.id.to_string(),
                    second: second.id.to_string(),
                });
            }
        }
    }

    Ok(keybinds)
}

fn lookup_in_section(
    id: &str,
    section_name: &str,
) -> Result<&'static KeybindAction, KeybindsError> {
    let action = action_by_id(id).ok_or_else(|| KeybindsError::UnknownAction {
        action: id.to_string(),
        section: section_name.to_string(),
    })?;
    if !action.section.name().eq_ignore_ascii_case(section_name) {
        return Err(KeybindsError::SectionMismatch {
            action: id.to_string(),
            section: section_name.to_string(),
            declared: action.section,
        });
    }
    Ok(action)
}

fn parse_configured(chord: &str, entry: &str, reserved: &[Chord]) -> Result<Chord, KeybindsError> {
    let parsed = Chord::parse(chord).map_err(|reason| KeybindsError::UnparseableChord {
        chord: chord.to_string(),
        entry: entry.to_string(),
        reason: reason.to_string(),
    })?;
    if reserved.contains(&parsed) {
        return Err(KeybindsError::ReservedChord {
            chord: chord.to_string(),
            entry: entry.to_string(),
        });
    }
    Ok(parsed)
}
