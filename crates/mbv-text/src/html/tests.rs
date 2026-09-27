use super::{decode_entities, html_to_text};

#[test]
fn decode_entities_known_entities() {
    assert_eq!(decode_entities("&quot;hi&quot;"), "\"hi\"");
    assert_eq!(decode_entities("it&apos;s"), "it's");
    assert_eq!(decode_entities("a &lt; b &gt; c"), "a < b > c");
    assert_eq!(decode_entities("a &amp; b"), "a & b");
}

#[test]
fn decode_entities_passthrough() {
    assert_eq!(decode_entities("plain text"), "plain text");
    assert_eq!(decode_entities(""), "");
}

#[test]
fn decode_entities_numeric_refs() {
    assert_eq!(decode_entities("&#38;"), "&");
    assert_eq!(decode_entities("&#x27;"), "'");
    assert_eq!(decode_entities("&#x27A1;"), "➡");
    // Unknown named/numeric refs and stray '&' are left untouched.
    assert_eq!(decode_entities("&unknown;"), "&unknown;");
    assert_eq!(decode_entities("a & b"), "a & b");
    assert_eq!(decode_entities("50% &amp;amp; chance"), "50% &amp; chance");
}

#[test]
fn html_to_text_paragraph_breaks_and_entities() {
    assert_eq!(
        html_to_text("<p>First paragraph</p><p>Second &amp; final</p>"),
        "First paragraph\nSecond & final"
    );
    // `<br/>` (self-closing and bare) makes a single line break.
    assert_eq!(html_to_text("Line one<br/>Line two"), "Line one\nLine two");
    // Adjacent block tags collapse to one newline, not blank lines.
    assert_eq!(html_to_text("<p>One</p><br/><p>Two</p>"), "One\nTwo");
}

#[test]
fn html_to_text_keeps_link_text_and_url() {
    assert_eq!(
        html_to_text(r#"<p>See <a href="https://example.test/a&amp;b">the article</a>.</p>"#),
        "See the article (https://example.test/a&b)."
    );
}

#[test]
fn html_to_text_drops_inline_formatting_and_images() {
    // Inline spans/strong keep their text; the whole `<img.../>` tag (with its
    // many attributes) is dropped entirely.
    assert_eq!(
        html_to_text(
            r#"<p><span style="font-weight: 400;">Rightwing &amp; left</span><img width="534" src="https://example.test/x.png" alt="" /></p>"#
        ),
        "Rightwing & left"
    );
}

#[test]
fn html_to_text_numeric_entities() {
    // Curly quotes and ellipses from the Novara feed decode via numeric refs.
    assert_eq!(
        html_to_text("&#8220;quoted&#8221; and &#8230;"),
        "\u{201c}quoted\u{201d} and \u{2026}"
    );
}

#[test]
fn html_to_text_plain_passthrough() {
    assert_eq!(html_to_text("plain text"), "plain text");
    assert_eq!(html_to_text(""), "");
    // A stray unknown tag is dropped from the text (it is markup).
    assert_eq!(html_to_text("no <tags> here"), "no here");
}
