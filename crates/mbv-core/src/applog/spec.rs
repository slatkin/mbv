use std::fmt::{Display, Formatter};

use tracing_subscriber::layer::Filter;

/// A default tracing level and target-specific overrides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSpec {
    pub default: tracing::level_filters::LevelFilter,
    pub directives: Vec<(String, tracing::level_filters::LevelFilter)>,
}

impl Default for LogSpec {
    fn default() -> Self {
        Self {
            default: tracing::level_filters::LevelFilter::INFO,
            directives: Vec::new(),
        }
    }
}

impl LogSpec {
    /// Parses comma-separated level and `target=level` directives.
    pub fn parse(spec: &str) -> Result<Self, LogSpecError> {
        let mut default = tracing::level_filters::LevelFilter::INFO;
        let mut directives = Vec::new();

        for raw in spec.split(',') {
            if raw.is_empty() {
                return Err(LogSpecError::new(spec));
            }
            if let Some((target, level)) = raw.split_once('=') {
                if target.is_empty() || target.contains(char::is_whitespace) || level.contains('=')
                {
                    return Err(LogSpecError::new(spec));
                }
                let level = parse_level(level).ok_or_else(|| LogSpecError::new(spec))?;
                if let Some((_, existing_level)) = directives
                    .iter_mut()
                    .find(|(existing, _)| existing == target)
                {
                    *existing_level = level;
                } else {
                    directives.push((target.to_owned(), level));
                }
            } else {
                default = parse_level(raw).ok_or_else(|| LogSpecError::new(spec))?;
            }
        }

        Ok(Self {
            default,
            directives,
        })
    }

    /// Returns the effective level for a target. Third-party module paths
    /// (`::` targets outside `mbv`/`mbv_*`) are capped at warn unless directed.
    #[must_use]
    pub fn level_for_target(&self, target: &str) -> tracing::level_filters::LevelFilter {
        self.directive_level_for_target(target).unwrap_or_else(|| {
            if target.contains("::") && !is_first_party(target) {
                self.default.min(tracing::level_filters::LevelFilter::WARN)
            } else {
                self.default
            }
        })
    }

    /// The most verbose level in the spec (default or any directive).
    pub(crate) fn most_verbose_level(&self) -> tracing::level_filters::LevelFilter {
        self.directives
            .iter()
            .map(|(_, level)| *level)
            .chain(std::iter::once(self.default))
            .max()
            .unwrap_or(tracing::level_filters::LevelFilter::OFF)
    }

    fn directive_level_for_target(
        &self,
        target: &str,
    ) -> Option<tracing::level_filters::LevelFilter> {
        self.directives
            .iter()
            .filter(|(prefix, _)| target_matches(target, prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, level)| *level)
    }
}

impl Display for LogSpec {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", level_name(self.default))?;
        for (target, level) in &self.directives {
            write!(f, ",{target}={}", level_name(*level))?;
        }
        Ok(())
    }
}

impl<S: tracing::Subscriber> Filter<S> for LogSpec {
    fn enabled(
        &self,
        meta: &tracing::Metadata<'_>,
        _: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        *meta.level() <= self.level_for_target(meta.target())
    }

    // The decision depends only on static metadata, so it is cacheable per callsite.
    fn callsite_enabled(
        &self,
        meta: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        if *meta.level() <= self.level_for_target(meta.target()) {
            tracing::subscriber::Interest::always()
        } else {
            tracing::subscriber::Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(self.most_verbose_level())
    }
}

/// An invalid log-level specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSpecError {
    spec: String,
}

impl LogSpecError {
    fn new(spec: &str) -> Self {
        Self {
            spec: spec.to_owned(),
        }
    }
}

impl Display for LogSpecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid log-level specification {:?}", self.spec)
    }
}

impl std::error::Error for LogSpecError {}

fn parse_level(level: &str) -> Option<tracing::level_filters::LevelFilter> {
    match level {
        "error" => Some(tracing::level_filters::LevelFilter::ERROR),
        "warn" => Some(tracing::level_filters::LevelFilter::WARN),
        "info" => Some(tracing::level_filters::LevelFilter::INFO),
        "debug" => Some(tracing::level_filters::LevelFilter::DEBUG),
        "trace" => Some(tracing::level_filters::LevelFilter::TRACE),
        _ => None,
    }
}

fn level_name(level: tracing::level_filters::LevelFilter) -> String {
    level.to_string().to_lowercase()
}

fn is_first_party(target: &str) -> bool {
    target == "mbv" || target.starts_with("mbv_")
}

fn target_matches(target: &str, prefix: &str) -> bool {
    target == prefix
        || target
            .strip_prefix(prefix)
            .is_some_and(|suffix| suffix.starts_with("::"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("info", Some((tracing::level_filters::LevelFilter::INFO, vec![])))]
    #[case(
        "warn,player=debug",
        Some((
            tracing::level_filters::LevelFilter::WARN,
            vec![("player".to_owned(), tracing::level_filters::LevelFilter::DEBUG)]
        ))
    )]
    #[case("trace", Some((tracing::level_filters::LevelFilter::TRACE, vec![])))]
    #[case("", None)]
    #[case(",info", None)]
    #[case("info,", None)]
    #[case("loud", None)]
    #[case("player=", None)]
    #[case("=debug", None)]
    #[case("player=debug=trace", None)]
    fn parse_directives(
        #[case] input: &str,
        #[case] expected: Option<(
            tracing::level_filters::LevelFilter,
            Vec<(String, tracing::level_filters::LevelFilter)>,
        )>,
    ) {
        let parsed = LogSpec::parse(input);
        match expected {
            Some((default, directives)) => {
                let spec = parsed.expect("valid log spec");
                assert_eq!(spec.default, default);
                assert_eq!(spec.directives, directives);
            }
            None => assert_eq!(parsed, Err(LogSpecError::new(input))),
        }
    }

    #[test]
    fn default_spec_is_info_with_no_directives() {
        assert_eq!(
            LogSpec::default(),
            LogSpec::parse("info").expect("valid spec")
        );
    }

    #[test]
    fn display_round_trips_through_parse() {
        let spec = LogSpec::parse("info,player=debug,player::load=trace").expect("valid spec");

        assert_eq!(LogSpec::parse(&spec.to_string()), Ok(spec));
    }

    #[test]
    fn longest_target_prefix_wins_on_component_boundary() {
        let spec = LogSpec::parse("info,player=warn,player::load=debug").expect("valid spec");

        assert_eq!(
            spec.level_for_target("player::load::item"),
            tracing::level_filters::LevelFilter::DEBUG
        );
        assert_eq!(
            spec.level_for_target("player::play"),
            tracing::level_filters::LevelFilter::WARN
        );
        assert_eq!(
            spec.level_for_target("player2"),
            tracing::level_filters::LevelFilter::INFO
        );
    }

    #[test]
    fn third_party_targets_are_warn_capped_unless_directed() {
        let spec = LogSpec::parse("debug,third_party=trace").expect("valid spec");

        assert_eq!(
            spec.level_for_target("rustls::client"),
            tracing::level_filters::LevelFilter::WARN
        );
        assert_eq!(
            spec.level_for_target("third_party::client"),
            tracing::level_filters::LevelFilter::TRACE
        );
        assert_eq!(
            spec.level_for_target("player"),
            tracing::level_filters::LevelFilter::DEBUG
        );
    }

    #[test]
    fn first_party_module_paths_are_not_capped() {
        let spec = LogSpec::parse("debug").expect("valid spec");

        assert_eq!(
            spec.level_for_target("mbv_daemon::control"),
            tracing::level_filters::LevelFilter::DEBUG
        );
    }

    #[test]
    fn error_has_display_and_error_contract() {
        let error = LogSpec::parse("player=").expect_err("invalid spec");
        let _: &dyn std::error::Error = &error;

        assert!(error.to_string().contains("player="));
    }
}
