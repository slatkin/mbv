//! Path-only rendering of a URL for logs (never the query string or userinfo).

use std::fmt;

/// Displays the path of an absolute URL string, or `<unparsable>` when the
/// input has no `scheme://authority` prefix, so a bad URL never logs as an
/// empty path.
#[derive(Debug)]
pub struct UrlPath<'a>(pub &'a str);

impl fmt::Display for UrlPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((_, rest)) = self.0.split_once("://") else {
            return f.write_str("<unparsable>");
        };
        let rest = rest.split(['?', '#']).next().unwrap_or_default();
        f.write_str(rest.find('/').map_or("/", |start| &rest[start..]))
    }
}

#[cfg(test)]
mod tests {
    use super::UrlPath;
    use rstest::rstest;

    #[rstest]
    #[case::path_without_query("http://host:8096/Items/1?api_key=secret", "/Items/1")]
    #[case::fragment("https://host/a/b#frag", "/a/b")]
    #[case::no_path("http://host?x=1", "/")]
    #[case::not_a_url("not a url", "<unparsable>")]
    fn renders_path_only(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(UrlPath(input).to_string(), expected);
    }
}
