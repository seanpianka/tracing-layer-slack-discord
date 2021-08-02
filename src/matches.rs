use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::io::Error;

use regex::Regex;

/// EventFilters describes two optional lists of regular expressions used to filter events.
///
/// If provided, each expression is used in either negatively ("does NOT MATCH") or
/// positively ("does MATCH") filter against a specified value.
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

/// Interpret and convert a single regex as a single positive filter and no negative filter.
impl From<Regex> for EventFilters {
    fn from(positive: Regex) -> Self {
        Self::new(Some(vec![positive]), None)
    }
}

/// Interpret and convert a pair of regex as a single positive filter and a single negative filter.
impl From<(Option<Regex>, Option<Regex>)> for EventFilters {
    fn from((single_positive, single_negative): (Option<Regex>, Option<Regex>)) -> Self {
        Self::new(single_positive.map(|sp| vec![sp]), single_negative.map(|sn| vec![sn]))
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
    PositiveMatchFailed,
    NegativeMatchFailed,
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

impl Matcher for EventFilters {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        if let Some(negative) = &self.negative {
            for filter in negative {
                if filter.is_match(value) {
                    return Err(MatchingError::NegativeMatchFailed);
                }
            }
        }
        if let Some(positive) = &self.positive {
            for filter in positive {
                if !filter.is_match(value) {
                    return Err(MatchingError::PositiveMatchFailed);
                }
            }
        }
        Ok(())
    }
}

impl Matcher for Option<EventFilters> {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        if let Some(matcher) = self {
            matcher.process(value)
        } else {
            Ok(())
        }
    }
}

impl Matcher for Vec<Regex> {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        for filter in self {
            if filter.is_match(value) {
                return Err(MatchingError::NegativeMatchFailed);
            }
        }
        Ok(())
    }
}

impl Matcher for Option<Vec<Regex>> {
    fn process(&self, value: &str) -> Result<(), MatchingError> {
        if let Some(matcher) = self {
            matcher.process(value)
        } else {
            Ok(())
        }
    }
}
