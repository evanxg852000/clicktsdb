use std::{error, io};

use thiserror::Error;

pub type FstResult<T> = std::result::Result<T, FtsError>;

#[derive(Error, Debug)]
pub enum FtsError {
    #[error("IO error")]
    Io(#[from] io::Error),
    #[error("Fst error")]
    Fst(#[from] fst::Error),
    #[error("Fst levenshtein error")]
    Levenshtein(#[from] fst::automaton::LevenshteinError),
    #[error("Fst regex error")]
    Regex(#[from] regex_automata::Error),
    #[error("Serde error")]
    Serde(#[from] bincode::Error),
    #[error("Query error")]
    Query,
    #[error("Query not supported.")]
    QueryNotSupported,

    #[error("IndexReader error")]
    IndexReader,
    #[error("Document not found")]
    DocNotFound,
    #[error("Other error")]
    Other(String),
}
