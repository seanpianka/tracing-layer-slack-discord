use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::io::Error;

use regex::Regex;

/// EventFilters describes two optional lists of regular expressions used to filter events.
///
/// If provided, each expression is used in either negatively (exclusion) or positively (inclusion)
/// matching fields of events. A match denotes logging the message to the configured Slack channel.
pub struct EventFilters {
    /// An optional list of one-or-more regular expressions to use for determining record inclusion.
    positive: Option<Vec<Regex>>,
    /// An optional list of one-or-more regular expressions to use for determining record exclusion.
    negative: Option<Vec<Regex>>,
}

impl EventFilters {
    /// Create a new set of matches.
    pub fn new(positive: Option<Vec<Regex>>, negative: Option<Vec<Regex>>) -> Self {
        Self { positive, negative }
    }
}

/// Interpret and convert a pair of regex as a single positive filter and a single negative filter.
impl From<(Option<Regex>, Option<Regex>)> for EventFilters {
    fn from((single_positive, single_negative): (Option<Regex>, Option<Regex>)) -> Self {
        Self::new(
            match single_positive {
                None => None,
                Some(sp) => Some(vec![sp]),
            },
            match single_negative {
                None => None,
                Some(sn) => Some(vec![sn]),
            },
        )
    }
}

/// Interpret and convert a pair of regex as a single positive filter and a single negative filter.
impl From<(Regex, Regex)> for EventFilters {
    fn from((single_positive, single_negative): (Regex, Regex)) -> Self {
        Self::from((Some(single_positive), Some(single_negative)))
    }
}

/// Interpret and convert a pair of lists of regex as positive and negative filters.
impl From<(Vec<Regex>, Vec<Regex>)> for EventFilters {
    fn from((positives, negatives): (Vec<Regex>, Vec<Regex>)) -> Self {
        Self::new(Some(positives), Some(negatives))
    }
}

pub(crate) enum MatchingError {
    TargetPositiveMatchFailed,
    TargetNegativeMatchFailed,
    MessagePositiveMatchFailed,
    MessageNegativeMatchFailed,
    IoError(std::io::Error),
    SerdeError(serde_json::Error),
}

impl From<std::io::Error> for MatchingError {
    fn from(e: Error) -> Self {
        MatchingError::IoError(e)
    }
}

impl From<serde_json::Error> for MatchingError {
    fn from(e: serde_json::Error) -> Self {
        MatchingError::SerdeError(e)
    }
}

pub(crate) trait Matcher {
    fn process(&self, value: &str) -> Result<(), MatchingError>;
}

impl Matcher for Option<EventFilters> {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        if let Some(matches) = self {
            matches.process(value)
        } else {
            Ok(())
        }
    }
}

impl Matcher for EventFilters {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        if let Some(positive) = &self.positive {
            for filter in positive {
                if !filter.is_match(value) {
                    return Err(MatchingError::TargetPositiveMatchFailed);
                }
            }
        }
        if let Some(negative) = &self.negative {
            for filter in negative {
                if filter.is_match(value) {
                    return Err(MatchingError::TargetNegativeMatchFailed);
                }
            }
        }
        Ok(())
    }
}
