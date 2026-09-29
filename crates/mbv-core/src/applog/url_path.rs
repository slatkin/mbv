//! Safe URL rendering for logs: omit userinfo and redact credential query values.

use std::fmt;

/// Displays a URL's path and non-sensitive query parameters without authority,
/// userinfo, or credential values. Invalid inputs render as `<unparsable>`.
#[derive(Debug)]
pub struct UrlPath<'a>(pub &'a str);

impl fmt::Display for UrlPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((_, rest)) = self.0.split_once("://") else {
            return f.write_str("<unparsable>");
        };
        let (url, _) = rest.split_once('#').unwrap_or((rest, ""));
        let (authority, query) = url.split_once('?').unwrap_or((url, ""));
        let path = authority.find('/').map_or("/", |start| &authority[start..]);
        f.write_str(path)?;
        if query.is_empty() {
            return Ok(());
        }
        f.write_str("?")?;
        for (index, parameter) in query.split('&').enumerate() {
            if index > 0 {
                f.write_str("&")?;
            }
            let (key, value) = parameter.split_once('=').unwrap_or((parameter, ""));
            f.write_str(key)?;
            if parameter.contains('=') {
                f.write_str("=")?;
                if is_credential_parameter(key) {
                    f.write_str("<redacted>")?;
                } else {
                    f.write_str(value)?;
                }
            }
        }
        Ok(())
    }
}

fn is_credential_parameter(key: &str) -> bool {
    [
        "api_key",
        "apikey",
        "token",
        "password",
        "x-emby-token",
        "access_token",
        "refresh_token",
    ]
    .iter()
    .any(|credential| key.eq_ignore_ascii_case(credential))
}

#[cfg(test)]
mod tests {
    use super::UrlPath;
    use rstest::rstest;

    #[rstest]
    #[case::redacts_api_key_and_preserves_other_params(
        "http://host:8096/Items/1?static=true&api_key=secret&MediaSourceId=msid1",
        "/Items/1?static=true&api_key=<redacted>&MediaSourceId=msid1"
    )]
    #[case::redacts_token_case_insensitively(
        "https://host/a?Token=secret&format=mp3",
        "/a?Token=<redacted>&format=mp3"
    )]
    #[case::fragment("https://host/a/b#frag", "/a/b")]
    #[case::no_path("http://host?x=1", "/?x=1")]
    #[case::not_a_url("not a url", "<unparsable>")]
    fn redacts_credentials_and_preserves_safe_query_parameters(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(UrlPath(input).to_string(), expected);
    }
}
