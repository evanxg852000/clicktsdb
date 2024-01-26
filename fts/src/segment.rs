use std::{
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
};

use bytes::Bytes;
use crossbeam::channel::Sender;
use hashbrown::{HashMap, HashSet};
use ulid::Ulid;

use crate::{
    doc_store::DocStore, error::FstResult, postings::Postings, query::Query, DocId, Document,
};

pub struct SegmentInfo {
    pub id: String,
    pub num_docs: usize,
    pub num_terms: usize,
    pub docs_size_bytes: usize,
}

pub(crate) struct WritableSegment {
    /// Segment id.
    id: String,
    /// Term dictionary mapping (term -> sorted_vec<doc_id>)
    terms: HashMap<String, Vec<DocId>>, // might change it to BtreeMap
    /// Document store
    documents: HashMap<DocId, Bytes>,
    /// Accumulated document size in bytes
    num_bytes: usize,
}

impl WritableSegment {
    pub fn insert(&mut self, documents: Vec<Document>) {
        for document in documents {
            for term in document.terms {
                self.terms
                    .entry(term.clone())
                    .and_modify(|posting_list| posting_list.push(document.id))
                    .or_insert(vec![document.id]);
            }
            let document_length = document.content.len();
            self.documents.insert(document.id, document.content);
            self.num_bytes += document_length;
        }
    }

    /// This operation can be expensive. We might need to run
    /// this in another thread to avoid blocking document
    /// ingestion.
    pub fn into_segment(self, index_directory: &PathBuf) -> FstResult<Segment> {
        //TODO: sort and remove duplicate in posting_list `self.terms`
        let segment_directory = index_directory.join(&self.id);
        if !segment_directory.exists() {
            fs::create_dir(segment_directory)?;
        }
        let postings = Postings::new(index_directory, &self.id, self.terms)?;
        let store = DocStore::new(index_directory, &self.id, self.documents)?;
        Ok(Segment {
            id: self.id,
            postings,
            store,
        })
    }

    pub fn num_docs(&self) -> usize {
        self.documents.len()
    }

    pub fn info(&self) -> SegmentInfo {
        SegmentInfo {
            id: self.id.clone(),
            num_docs: self.documents.len(),
            num_terms: self.terms.len(),
            docs_size_bytes: self.num_bytes,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Segment {
    id: String,
    postings: Postings,
    store: DocStore,
}

impl Segment {
    pub fn create() -> WritableSegment {
        WritableSegment {
            id: Ulid::new().to_string(),
            terms: HashMap::new(),
            documents: HashMap::new(),
            num_bytes: 0,
        }
    }

    pub fn open(index_directory: &PathBuf, segment_id: &str) -> FstResult<Self> {
        let postings = Postings::open(index_directory, segment_id)?;
        let store = DocStore::open(index_directory, segment_id)?;
        Ok(Self {
            id: segment_id.to_string(),
            postings,
            store,
        })
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone)]
pub struct SegmentReader {
    segment: Arc<Segment>,
}

impl SegmentReader {
    pub(crate) fn new(segment: Arc<Segment>) -> Self {
        Self { segment }
    }

    /// List all the terms matching within a term range.
    pub fn list_terms_in_range(&self, from: &str, to: &str) -> FstResult<Vec<String>> {
        self.segment
            .postings
            .range(from, to)
            .map(|postings_info| postings_info.into_iter().map(|(term, _)| term).collect())
    }

    /// Search and returns matching doc_ids within a term range.
    pub fn search_in_range(&self, from: &str, to: &str) -> FstResult<Vec<DocId>> {
        let posting_info = self.segment.postings.range(from, to)?;
        fetch_doc_ids(&self.segment.postings, posting_info)
    }

    /// List all the terms matching a query.
    pub fn list_terms(&self, query: &Query) -> FstResult<Vec<String>> {
        let matcher = query.matcher()?;
        self.segment
            .postings
            .search(matcher)
            .map(|postings_info| postings_info.into_iter().map(|(term, _)| term).collect())
    }

    /// Search and returns doc_ids matching a query.
    pub fn search(&self, query: &Query) -> FstResult<Vec<DocId>> {
        evaluate_query(&self.segment.postings, query)
    }

    /// Get the content of a document with the provided DocId.
    pub fn fetch_doc(&self, id: DocId) -> FstResult<Option<Bytes>> {
        self.segment.store.fetch_doc(id)
    }

    // fn fetch_doc_ids(&self, posting_info: Vec<(String, u64)>) -> FstResult<Vec<DocId>> {
    //     let mut doc_ids = HashSet::new();
    //     for (_, offset) in posting_info {
    //         let ids = self.segment.postings.postings(offset as usize)?;
    //         for id in ids {
    //             doc_ids.insert(id);
    //         }
    //     }
    //     Ok(doc_ids.into_iter().collect())
    // }
}

pub(crate) type CommitReplySender = oneshot::Sender<FstResult<()>>;

pub(crate) struct SegmentFinalizer {
    segment_sender: Sender<(WritableSegment, Option<CommitReplySender>)>,
    join_handle: JoinHandle<FstResult<()>>,
}

impl SegmentFinalizer {
    pub fn start(index_directory: &PathBuf, segments: Arc<RwLock<Vec<Arc<Segment>>>>) -> Self {
        let (segment_sender, segment_receiver) =
            crossbeam::channel::bounded::<(WritableSegment, Option<CommitReplySender>)>(10);
        let moved_index_directory = index_directory.clone();
        let join_handle = thread::spawn(move || {
            for (writable_segment, reply_sender_opt) in segment_receiver.iter() {
                let segment = writable_segment.into_segment(&moved_index_directory)?;
                let mut segment_lock = segments.write().unwrap();
                segment_lock.push(Arc::new(segment));
                drop(segment_lock);
                if let Some(reply_sender) = reply_sender_opt {
                    reply_sender.send(Ok(())).unwrap();
                }
            }
            Ok(())
        });

        Self {
            segment_sender,
            join_handle,
        }
    }

    pub fn finalize(
        &self,
        segment: WritableSegment,
        commit_reply_sender_opt: Option<CommitReplySender>,
    ) {
        // do not bother finalizing empty segment.
        if segment.num_docs() > 0 {
            self.segment_sender
                .send((segment, commit_reply_sender_opt))
                .unwrap();
        }
    }

    pub fn stop(self) -> FstResult<()> {
        drop(self.segment_sender);
        self.join_handle.join().unwrap()
    }
}

fn evaluate_query(postings: &Postings, query: &Query) -> FstResult<Vec<DocId>> {
    match query {
        Query::Or(left, right) => {
            let mut left_doc_ids = evaluate_query(postings, left)?.into_iter().peekable();
            let mut right_doc_ids = evaluate_query(postings, right)?.into_iter().peekable();

            // perform union (OR)
            let mut result = Vec::with_capacity(left_doc_ids.len() + right_doc_ids.len());
            loop {
                let (Some(left_v), Some(right_v)) = (left_doc_ids.peek(), right_doc_ids.peek())
                else {
                    break;
                };

                if left_v < right_v {
                    result.push(left_doc_ids.next().unwrap());
                } else {
                    result.push(right_doc_ids.next().unwrap());
                }
            }

            while let Some(left_v) = left_doc_ids.next() {
                result.push(left_v);
            }

            while let Some(right_v) = right_doc_ids.next() {
                result.push(right_v);
            }

            Ok(result)
        }
        Query::And(left, right) => {
            let mut left_doc_ids = evaluate_query(postings, left)?.into_iter().peekable();
            let mut right_doc_ids = evaluate_query(postings, right)?.into_iter().peekable();

            // perform intersection (AND)
            let mut result = vec![];
            loop {
                let (Some(left_v), Some(right_v)) = (left_doc_ids.peek(), right_doc_ids.peek())
                else {
                    break;
                };

                if left_v < right_v {
                    left_doc_ids.next();
                } else if left_v > right_v {
                    right_doc_ids.next();
                } else {
                    if left_v == right_v {
                        result.push(*left_v);
                    }
                    left_doc_ids.next();
                    right_doc_ids.next();
                }
            }

            Ok(result)
        }
        query => {
            let posting_info = postings.search(query.matcher()?)?;
            fetch_doc_ids(postings, posting_info)
        }
    }
}

fn fetch_doc_ids(postings: &Postings, posting_info: Vec<(String, u64)>) -> FstResult<Vec<DocId>> {
    let mut doc_ids = HashSet::new();
    for (_, offset) in posting_info {
        let ids = postings.postings(offset as usize)?;
        for id in ids {
            doc_ids.insert(id);
        }
    }
    Ok(doc_ids.into_iter().collect())
}
