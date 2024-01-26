use crate::{
    error::{FstResult, FtsError},
    matcher::Matcher,
};

#[derive(Debug)]
pub enum Query {
    All,
    Equal(String),
    NotEqual(String),
    StartsWith(String),
    NotStartsWith(String),
    Fuzzy(String, u32),
    NotFuzzy(String, u32),
    Regex(String),
    NotRegex(String),
    Or(Box<Query>, Box<Query>),
    And(Box<Query>, Box<Query>),
}

impl Query {
    pub(crate) fn matcher(&self) -> FstResult<Matcher> {
        match self {
            Query::All => Ok(Matcher::all(false)),
            Query::Equal(term) => Ok(Matcher::equal(&term, false)),
            Query::NotEqual(term) => Ok(Matcher::equal(&term, true)),
            Query::StartsWith(term) => Ok(Matcher::starts_with(term, false)),
            Query::NotStartsWith(term) => Ok(Matcher::starts_with(term, true)),
            Query::Fuzzy(term, distance) => Ok(Matcher::fuzzy(term, *distance, false)),
            Query::NotFuzzy(term, distance) => Ok(Matcher::fuzzy(term, *distance, true)),
            Query::Regex(pattern) => Ok(Matcher::regex(pattern, false)),
            Query::NotRegex(pattern) => Ok(Matcher::regex(pattern, true)),
            _ => Err(FtsError::QueryNotSupported),
        }
    }
}

// TODO: add query builder
