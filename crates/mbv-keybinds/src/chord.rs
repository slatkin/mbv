/// Keyboard modifiers of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyMods(u8);

impl KeyMods {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// The key half of a chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Backspace,
    Enter,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
}

/// One normalized keyboard chord: modifiers plus key. The textual grammar is
/// `"Ctrl+b"`, `"F8"`, `"Shift+Left"`, `"Space"`; modifier order is not
/// significant and the canonical rendering (Display) is Ctrl+Shift+Alt+key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: KeyMods,
    pub key: Key,
}

/// Why a chord string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChordParseError {
    Empty,
    EmptyToken,
    UnknownModifier(String),
    UnknownKey(String),
}

impl std::fmt::Display for ChordParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "empty chord"),
            Self::EmptyToken => write!(f, "empty chord token"),
            Self::UnknownModifier(name) => write!(f, "unknown modifier `{name}`"),
            Self::UnknownKey(name) => write!(f, "unknown key `{name}`"),
        }
    }
}

impl Chord {
    /// Parse a chord string: optional modifiers (`Ctrl`, `Shift`, `Alt`,
    /// order-insensitive, case-insensitive) plus one key (a single character,
    /// `F1`–`F12`, or a named key such as `Esc`, `Tab`, `BackTab`, `Enter`,
    /// `Space`, arrows, `Home`, `End`, `PageUp`, `PageDown`, `Delete`,
    /// `Insert`, `Backspace`). A lone `+` or `-` is a character chord.
    ///
    /// # Panics
    ///
    /// Panics if the `+`-split has no last token, i.e. if the "a non-empty
    /// trimmed string splits into at least one token" invariant is violated.
    /// `s` is already known non-empty here, so the split always yields one.
    pub fn parse(s: &str) -> Result<Self, ChordParseError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ChordParseError::Empty);
        }
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(Self {
                mods: KeyMods::NONE,
                key: Key::Char(c),
            });
        }
        let tokens: Vec<&str> = s.split('+').collect();
        let (key_token, modifier_tokens) = tokens.split_last().expect("non-empty split");
        let mut mods = KeyMods::NONE;
        for token in modifier_tokens {
            let token = token.trim();
            if token.is_empty() {
                return Err(ChordParseError::EmptyToken);
            }
            let modifier = match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyMods::CTRL,
                "shift" => KeyMods::SHIFT,
                "alt" => KeyMods::ALT,
                _ => {
                    return Err(ChordParseError::UnknownModifier(token.to_string()));
                }
            };
            mods = mods.union(modifier);
        }
        Ok(Self {
            mods,
            key: parse_key(key_token)?,
        })
    }
}

fn parse_key(token: &str) -> Result<Key, ChordParseError> {
    const NAMED_KEYS: &[(&str, Key)] = &[
        ("esc", Key::Esc),
        ("escape", Key::Esc),
        ("tab", Key::Tab),
        ("backtab", Key::BackTab),
        ("enter", Key::Enter),
        ("return", Key::Enter),
        ("space", Key::Char(' ')),
        ("left", Key::Left),
        ("right", Key::Right),
        ("up", Key::Up),
        ("down", Key::Down),
        ("home", Key::Home),
        ("end", Key::End),
        ("pageup", Key::PageUp),
        ("pagedown", Key::PageDown),
        ("delete", Key::Delete),
        ("del", Key::Delete),
        ("insert", Key::Insert),
        ("backspace", Key::Backspace),
    ];

    let token = token.trim();
    if token.is_empty() {
        return Err(ChordParseError::EmptyToken);
    }
    let mut chars = token.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(Key::Char(c));
    }
    let lower = token.to_ascii_lowercase();
    if let Some((_, key)) = NAMED_KEYS.iter().find(|(name, _)| *name == lower) {
        return Ok(*key);
    }
    lower
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
        .filter(|number| (1..=12).contains(number))
        .map(Key::F)
        .ok_or_else(|| ChordParseError::UnknownKey(token.to_string()))
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backspace => write!(f, "Backspace"),
            Self::Enter => write!(f, "Enter"),
            Self::Esc => write!(f, "Esc"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Up => write!(f, "Up"),
            Self::Down => write!(f, "Down"),
            Self::Home => write!(f, "Home"),
            Self::End => write!(f, "End"),
            Self::PageUp => write!(f, "PageUp"),
            Self::PageDown => write!(f, "PageDown"),
            Self::Tab => write!(f, "Tab"),
            Self::BackTab => write!(f, "BackTab"),
            Self::Delete => write!(f, "Delete"),
            Self::Insert => write!(f, "Insert"),
            Self::F(n) => write!(f, "F{n}"),
            Self::Char(' ') => write!(f, "Space"),
            Self::Char(c) => write!(f, "{c}"),
        }
    }
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mods.contains(KeyMods::CTRL) {
            write!(f, "Ctrl+")?;
        }
        if self.mods.contains(KeyMods::SHIFT) {
            write!(f, "Shift+")?;
        }
        if self.mods.contains(KeyMods::ALT) {
            write!(f, "Alt+")?;
        }
        write!(f, "{}", self.key)
    }
}
