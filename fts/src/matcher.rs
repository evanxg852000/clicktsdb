/// Matcher is the core ast of all queries
#[derive(Debug)]
pub(crate) struct Matcher<'a> {
    pub term_matcher: TermMatcher<'a>,
    pub complement: bool,
}

impl<'a> Matcher<'a> {
    pub fn all(complement: bool) -> Self {
        Self {
            term_matcher: TermMatcher::All,
            complement,
        }
    }

    pub fn equal(term: &'a str, complement: bool) -> Self {
        Self {
            term_matcher: TermMatcher::Equal(term),
            complement,
        }
    }

    pub fn starts_with(term: &'a str, complement: bool) -> Self {
        Self {
            term_matcher: TermMatcher::StartsWith(term),
            complement,
        }
    }

    pub fn fuzzy(term: &'a str, distance: u32, complement: bool) -> Self {
        Self {
            term_matcher: TermMatcher::Fuzzy(term, distance),
            complement,
        }
    }

    pub fn regex(pattern: &'a str, complement: bool) -> Self {
        Self {
            term_matcher: TermMatcher::Regex(pattern),
            complement,
        }
    }
}

#[derive(Debug)]
pub(crate) enum TermMatcher<'a> {
    All,
    Equal(&'a str),
    StartsWith(&'a str),
    Fuzzy(&'a str, u32),
    Regex(&'a str),
}

// DCC app is ok and running on test, it needs to be deployed
// talk to @andrea about date field 11h - 13h
// Next up is MGP
