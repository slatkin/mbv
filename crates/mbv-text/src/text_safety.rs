pub fn is_control_char(ch: char) -> bool {
    ch.is_ascii_control() || matches!(ch, '\u{80}'..='\u{9f}')
}
