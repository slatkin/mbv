//! Parses and applies the application's default and target-specific log levels.
//!
//! `LogSpec` accepts comma-separated level directives and implements the tracing
//! subscriber filter. Use `parse` for user input and the accessors to inspect a
//! validated specification.

use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};

use tracing_subscriber::layer::Filter;

/// A default tracing level and target-specific overrides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogSpec {
    default: tracing::level_filters::LevelFilter,
    directives: Vec<(String, tracing::level_filters::LevelFilter)>,
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
    /// The default level applied when no target directive matches.
    #[must_use]
    pub fn default_level(&self) -> tracing::level_filters::LevelFilter {
        self.default
    }

    /// Target-specific level directives in input order.
    #[must_use]
    pub fn directives(&self) -> &[(String, tracing::level_filters::LevelFilter)] {
        &self.directives
    }

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
        write!(f, "{}", self.default)?;
        for (target, level) in &self.directives {
            write!(f, ",{target}={level}")?;
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
#[derive(Debug)]
pub struct LogSpecError {
    spec: String,
    backtrace: Backtrace,
}

impl LogSpecError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    fn new(spec: &str) -> Self {
        Self {
            spec: spec.to_owned(),
            backtrace: Backtrace::capture(),
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
    #[case("info", tracing::level_filters::LevelFilter::INFO, vec![])]
    #[case(
        "warn,player=debug",
        tracing::level_filters::LevelFilter::WARN,
        vec![("player".to_owned(), tracing::level_filters::LevelFilter::DEBUG)]
    )]
    #[case("trace", tracing::level_filters::LevelFilter::TRACE, vec![])]
    fn parse_accepts_directives(
        #[case] input: &str,
        #[case] expected_default: tracing::level_filters::LevelFilter,
        #[case] expected_directives: Vec<(String, tracing::level_filters::LevelFilter)>,
    ) {
        let spec = LogSpec::parse(input).expect("case table should contain valid log specs");

        assert_eq!(spec.default_level(), expected_default);
        assert_eq!(spec.directives(), expected_directives);
    }

    #[rstest]
    #[case("")]
    #[case(",info")]
    #[case("info,")]
    #[case("loud")]
    #[case("player=")]
    #[case("=debug")]
    #[case("player=debug=trace")]
    fn parse_rejects_malformed_directives(#[case] input: &str) {
        let error = LogSpec::parse(input).expect_err("malformed spec must be rejected");

        assert_eq!(
            error.to_string(),
            format!("invalid log-level specification {input:?}")
        );
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

        let reparsed = LogSpec::parse(&spec.to_string()).expect("rendered spec must parse");

        assert_eq!(reparsed, spec);
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
        let spec = LogSpec::parse("trace,third_party=trace").expect("valid spec");

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
            tracing::level_filters::LevelFilter::TRACE
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
}
