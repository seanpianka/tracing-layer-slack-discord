use regex::Regex;

pub trait Filter {
    fn process(&self, value: &str) -> Result<(), FilterError>;
}

/// Keeps a value only when every positive pattern matches and no negative pattern matches.
#[derive(Debug, Clone, Default)]
pub struct EventFilters {
    /// Patterns that must all match.
    positive: Option<Vec<Regex>>,
    /// Patterns that must not match.
    negative: Option<Vec<Regex>>,
}

impl EventFilters {
    /// Creates filters from optional positive and negative pattern lists.
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

/// Uses one pattern as the required positive match.
impl From<Regex> for EventFilters {
    fn from(positive: Regex) -> Self {
        Self::new(Some(vec![positive]), None)
    }
}

/// Uses at most one positive pattern and one negative pattern.
impl From<(Option<Regex>, Option<Regex>)> for EventFilters {
    fn from((single_positive, single_negative): (Option<Regex>, Option<Regex>)) -> Self {
        Self::new(single_positive.map(|sp| vec![sp]), single_negative.map(|sn| vec![sn]))
    }
}

/// Uses the first pattern for inclusion and the second for exclusion.
impl From<(Regex, Regex)> for EventFilters {
    fn from((single_positive, single_negative): (Regex, Regex)) -> Self {
        Self::from((Some(single_positive), Some(single_negative)))
    }
}

/// Uses the first list for inclusion and the second for exclusion.
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

pub enum FilterError {
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
