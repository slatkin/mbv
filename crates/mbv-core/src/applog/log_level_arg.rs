//! The `--log-level` command-line argument: a `LogSpec` read from a raw argv.

use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};

use super::spec::{LogSpec, LogSpecError};

/// A rejected `--log-level` argument: the flag with no value, or a value that
/// `LogSpec` will not parse.
#[derive(Debug)]
pub struct LogLevelArgError {
    message: String,
    backtrace: Backtrace,
}

impl LogLevelArgError {
    /// Backtrace captured when this error was created.
    #[must_use]
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    fn missing_value() -> Self {
        Self {
            message: String::from(
                "mbv: --log-level requires a level or level[,target=level...] list",
            ),
            backtrace: Backtrace::capture(),
        }
    }

    fn invalid_spec(error: &LogSpecError) -> Self {
        Self {
            message: format!("mbv: {error}"),
            backtrace: Backtrace::capture(),
        }
    }
}

impl Display for LogLevelArgError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LogLevelArgError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// Reads `--log-level <spec>` from `args`, returning the last one given, or
/// `None` when the flag is absent.
pub fn parse_log_level_arg(args: &[String]) -> Result<Option<LogSpec>, LogLevelArgError> {
    let mut spec = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--log-level" {
            i += 1;
            let Some(value) = args.get(i) else {
                return Err(LogLevelArgError::missing_value());
            };
            spec = Some(
                LogSpec::parse(value).map_err(|error| LogLevelArgError::invalid_spec(&error))?,
            );
        }
        i += 1;
    }
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[rstest::rstest]
    #[case("error", "error")]
    #[case("warn", "warn")]
    #[case("info", "info")]
    #[case("debug", "debug")]
    #[case("trace", "trace")]
    #[case("info,player=debug", "info,player=debug")]
    fn log_level_arg_accepts_supported_values(#[case] value: &str, #[case] expected: &str) {
        let spec = parse_log_level_arg(&["--log-level".into(), value.into()])
            .expect("a supported value parses")
            .expect("the flag yields a spec");
        assert_eq!(spec.to_string(), expected);
    }

    #[rstest::rstest]
    #[case(vec!["--log-level".into(), "info,player=".into()])]
    #[case(vec!["--log-level".into(), "loud".into()])]
    #[case(vec!["--log-level".into()])]
    fn log_level_arg_rejects_invalid_values(#[case] args: Vec<String>) {
        parse_log_level_arg(&args).unwrap_err();
    }

    #[test]
    fn log_level_arg_is_absent_without_flag() {
        assert_eq!(parse_log_level_arg(&[]).ok().flatten(), None);
    }

    #[test]
    fn missing_value_reports_the_flag_in_its_message() {
        let error = parse_log_level_arg(&["--log-level".into()]).expect_err("no value follows");
        assert!(error.to_string().contains("--log-level requires"));
    }
}
