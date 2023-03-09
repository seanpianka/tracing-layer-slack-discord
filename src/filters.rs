use core::option::Option;
use core::option::Option::Some;
use core::result::Result;
use core::result::Result::{Err, Ok};
use std::io::Error;

use regex::Regex;

pub(crate) trait Filter {
    fn process(&self, value: &str) -> Result<(), FilterError>;
}

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

impl Filter for EventFilters {
    fn process(&self, value: &str) -> Result<(), FilterError> {
        if let Some(negative) = &self.negative {
            for filter in negative {
                if filter.is_match(value) {
                    return Err(FilterError::NegativeMatchFailed);
                }
            }
        }
        if let Some(positive) = &self.positive {
            for filter in positive {
                if !filter.is_match(value) {
                    return Err(FilterError::PositiveFilterFailed);
                }
            }
        }
        Ok(())
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

impl Filter for Option<EventFilters> {
    fn process(&self, value: &str) -> Result<(), FilterError> {
        if let Some(filter) = self {
            filter.process(value)
        } else {
            Ok(())
        }
    }
}

impl Filter for Vec<Regex> {
    fn process(&self, value: &str) -> Result<(), FilterError> {
        for filter in self {
            if filter.is_match(value) {
                return Err(FilterError::NegativeMatchFailed);
            }
        }
        Ok(())
    }
}

impl Filter for Option<Vec<Regex>> {
    fn process(&self, value: &str) -> Result<(), FilterError> {
        if let Some(matcher) = self {
            matcher.process(value)
        } else {
            Ok(())
        }
    }
}

pub(crate) enum FilterError {
    PositiveFilterFailed,
    NegativeMatchFailed,
    IoError(Box<dyn std::error::Error>),
    SerdeError(serde_json::Error),
}

impl From<Box<dyn std::error::Error>> for FilterError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        FilterError::IoError(e)
    }
}

impl From<serde_json::Error> for FilterError {
    fn from(e: serde_json::Error) -> Self {
        FilterError::SerdeError(e)
    }
}
