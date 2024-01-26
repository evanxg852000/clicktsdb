use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};

use hashbrown::HashMap;
use memmap2::Mmap;

use fst::{
    automaton::{AlwaysMatch, Levenshtein, Str},
    Automaton, IntoStreamer, Map, MapBuilder,
};

use crate::{
    error::{FstResult, FtsError},
    matcher::{Matcher, TermMatcher},
    segment_component_file, DocId, SegmentComponent,
};

const U64_NUM_BYTE: usize = 8;

/// A postings or inverted index to map term -> sorted_vec<doc_id>.
/// It has two stored components of files (.term, .post)
/// - .post: A file storing the sorted list of doc_id
/// - .term: A file storing the FST and mapping term -> offset in .post file
///
#[derive(Debug)]
pub(crate) struct Postings {
    // file_name: PathBuf,
    term_dictionary: Map<Mmap>,

    /// Posting List Format: sorted list of doc_id
    /// ┌────────────┬─────┬────┬────┬────┬─────────┐
    /// │# data-size │ ... │ .. │ .. │ .. │  item N │
    /// └────────────┴─────┴────┴────┴────┴─────────┘
    ///
    posting_list: Mmap,
}

impl Postings {
    pub fn new(
        index_directory: &PathBuf,
        segment_id: &str,
        mut term_dictionary: HashMap<String, Vec<DocId>>,
    ) -> FstResult<Self> {
        let posting_list_file_name =
            segment_component_file(index_directory, segment_id, SegmentComponent::PostingList);
        let posting_list_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(posting_list_file_name)?;
        let mut posting_list_writer = io::BufWriter::new(&posting_list_file);

        let term_dictionary_file_name = segment_component_file(
            index_directory,
            segment_id,
            SegmentComponent::TermDictionary,
        );
        let term_dictionary_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(term_dictionary_file_name)?;
        let term_dictionary_writer = io::BufWriter::new(&term_dictionary_file);
        // Create the fst builder to insert new term->posting_offset pairs.
        let mut term_dictionary_builder = MapBuilder::new(term_dictionary_writer)?;
        // sort the terms as fst only accepts in-order insertion
        let mut terms: Vec<(&[u8], &[DocId])> = term_dictionary
            .iter_mut()
            .map(|(term, list)| {
                list.sort();
                (term.as_bytes(), list.as_slice())
            })
            .collect();
        terms.sort_unstable_by_key(|(term, _)| *term);

        let mut offset = 0;
        for (term, list) in terms {
            term_dictionary_builder.insert(term, offset)?;
            let posting_list_bytes = bincode::serialize(&list)?;
            let posting_list_bytes_size = posting_list_bytes.len() as u64;
            posting_list_writer.write_all(&posting_list_bytes_size.to_le_bytes())?;
            posting_list_writer.write_all(&posting_list_bytes)?;
            offset += (U64_NUM_BYTE + posting_list_bytes.len()) as u64;
        }
        posting_list_writer.flush()?;
        term_dictionary_builder.finish()?;

        let term_dictionary_mmap_file = unsafe { Mmap::map(&term_dictionary_file)? };
        let term_dictionary = Map::new(term_dictionary_mmap_file)?;
        let posting_list_mmap_file = unsafe { Mmap::map(&posting_list_file)? };
        Ok(Postings {
            term_dictionary,
            posting_list: posting_list_mmap_file,
        })
    }

    pub fn open(index_directory: &PathBuf, segment_id: &str) -> FstResult<Self> {
        let posting_list_file_name =
            segment_component_file(index_directory, segment_id, SegmentComponent::PostingList);
        let posting_list = unsafe { Mmap::map(&File::open(posting_list_file_name)?)? };

        let term_dictionary_file_name = segment_component_file(
            index_directory,
            segment_id,
            SegmentComponent::TermDictionary,
        );
        let term_dictionary_mmap_file =
            unsafe { Mmap::map(&File::open(term_dictionary_file_name)?)? };
        let term_dictionary = Map::new(term_dictionary_mmap_file)?;

        Ok(Postings {
            term_dictionary,
            posting_list,
        })
    }

    pub fn range(&self, from: &str, to: &str) -> FstResult<Vec<(String, u64)>> {
        self.term_dictionary
            .range()
            .ge(from)
            .lt(to)
            .into_stream()
            .into_str_vec()
            .map_err(FtsError::from)
    }

    pub fn search(&self, matcher: Matcher) -> FstResult<Vec<(String, u64)>> {
        let complement = matcher.complement;
        match matcher.term_matcher {
            TermMatcher::All => self.search_automaton(AlwaysMatch, complement),
            TermMatcher::Equal(term) => self.search_automaton(Str::new(term), complement),
            TermMatcher::StartsWith(term) => {
                self.search_automaton(Str::new(term).starts_with(), complement)
            }
            TermMatcher::Fuzzy(term, dist) => {
                self.search_automaton(Levenshtein::new(term, dist)?, complement)
            }
            TermMatcher::Regex(pattern) => {
                let dfa = regex_automata::dense::Builder::new()
                    .anchored(true)
                    .build(pattern)?;
                self.search_automaton(dfa, complement)
            }
        }
    }

    pub fn postings(&self, offset: usize) -> FstResult<Vec<DocId>> {
        let mut data_size_bytes = [0u8; U64_NUM_BYTE];
        data_size_bytes.clone_from_slice(&self.posting_list[offset..offset + U64_NUM_BYTE]);
        let data_size = u64::from_le_bytes(data_size_bytes) as usize;

        let posting_list_bytes =
            &self.posting_list[offset + U64_NUM_BYTE..offset + U64_NUM_BYTE + data_size];
        let posting_list = bincode::deserialize::<Vec<u64>>(posting_list_bytes)?;
        Ok(posting_list)
    }

    fn search_automaton<A: Automaton>(
        &self,
        aut: A,
        complement: bool,
    ) -> FstResult<Vec<(String, u64)>> {
        if complement {
            return self
                .term_dictionary
                .search(aut.complement())
                .into_stream()
                .into_str_vec()
                .map_err(FtsError::from);
        }

        self.term_dictionary
            .search(aut)
            .into_stream()
            .into_str_vec()
            .map_err(FtsError::from)
    }
}
