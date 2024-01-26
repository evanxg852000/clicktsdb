// use std::collections::HashMap;

use std::{
    collections::{BTreeSet, BinaryHeap, VecDeque},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    thread::{self, JoinHandle},
    time::Duration,
};

use bytes::Bytes;
use crossbeam::channel::{Receiver, Sender};

use crate::{
    error::{FstResult, FtsError},
    query::Query,
    segment::{CommitReplySender, Segment, SegmentFinalizer, SegmentReader, WritableSegment},
};
// use crate::pos Postings;

pub type DocId = u64;

pub struct Document {
    pub id: DocId,
    pub content: Bytes,
    pub terms: Vec<String>,
}

impl Document {
    //TODO: build a macro for this
    pub fn new<T: ToString>(id: DocId, content: &str, terms: &[T]) -> Self {
        Self {
            id,
            content: Bytes::copy_from_slice(content.as_bytes()),
            terms: terms.iter().map(|s| s.to_string()).collect(),
        }
    }
}

pub enum SegmentComponent {
    DocStore,
    TermDictionary,
    PostingList,
    Metadata,
}

impl SegmentComponent {
    pub fn file_name(&self) -> &str {
        match self {
            SegmentComponent::DocStore => "docs.store",
            SegmentComponent::TermDictionary => "terms.dict",
            SegmentComponent::PostingList => "postings.list",
            SegmentComponent::Metadata => "meta.data",
        }
    }
}

pub fn segment_component_file(
    index_dir: &PathBuf,
    segment_id: &str,
    segment_component: SegmentComponent,
) -> PathBuf {
    index_dir
        .join(segment_id)
        .join(format!("{}", segment_component.file_name()))
}

/// An mmaped file backed storage for documents.
/// document can be compressed and should remain transparent to higher level.
// struct DocStore {
//     file_name: String,
//     documents: HashMap<DocId, DocStoreInfo>,
//     file_map: Mmap,
// }

// impl DocStore {
//     pub fn open_read() {}

//     pub fn open_write() {}
// }

#[derive(Debug)]
pub struct Config {
    pub directory: PathBuf,
}

impl Config {
    pub fn new(dir: &Path) -> Self {
        Self {
            directory: dir.to_path_buf(),
        }
    }
}

pub struct Index {
    config: Arc<Config>,
    current_segment: WritableSegment,
    segments: Arc<RwLock<Vec<Arc<Segment>>>>,
    handle: IndexerHandle,
}

impl Index {
    pub fn open(config: Config) -> FstResult<Self> {
        let directory = config.directory.clone();
        let mut segments = Vec::new();
        if !directory.as_path().exists() {
            fs::create_dir(&directory)?;
        }

        let paths = fs::read_dir(&directory)?;
        for path in paths {
            match path {
                Ok(entry) if entry.metadata()?.is_dir() => {
                    // Skip all segment with invalid ulid id.
                    let segment_id = entry.file_name();
                    let segment_id_str = segment_id.to_str().unwrap();
                    if ulid::Ulid::from_string(segment_id_str).is_err() {
                        println!("Ignoring segment with invalid id `{}`.", segment_id_str);
                        continue;
                    };

                    let segment = Segment::open(&directory, segment_id_str)?;
                    segments.push(Arc::new(segment));
                }
                _ => continue,
            }
        }

        // Sort segments in ascending order of creation,
        // knowing that segment id is ulid ordered
        segments.sort_by_key(|segment| segment.get_id().to_string());

        let config = Arc::new(config);
        let segments = Arc::new(RwLock::new(segments));

        let moved_config = config.clone();
        let moved_segments = segments.clone();

        let (task_command_sender, task_command_receiver) = crossbeam::channel::bounded(100);
        let task_join_handle = thread::spawn(move || {
            indexing_task(moved_config, moved_segments, task_command_receiver)
        });

        Ok(Self {
            config,
            current_segment: Segment::create(),
            segments,
            handle: IndexerHandle {
                join_handle: task_join_handle,
                ops_sender: task_command_sender,
            },
        })
    }

    pub fn writer(&self) -> IndexWriter {
        IndexWriter {
            operation_sender: self.handle.ops_sender.clone(),
        }
    }

    pub fn reader(&self) -> IndexReader {
        let segments_lock = self.segments.read().unwrap();
        let segment_readers = segments_lock
            .iter()
            .cloned()
            .map(|segment| SegmentReader::new(segment))
            .collect();
        IndexReader { segment_readers }
    }

    pub fn close(self, commit: bool) -> FstResult<()> {
        self.handle
            .ops_sender
            .send(IndexingOp::Shutdown(commit))
            .unwrap();
        self.handle.join_handle.join().unwrap()
    }
}

struct IndexerHandle {
    join_handle: JoinHandle<FstResult<()>>,
    ops_sender: Sender<IndexingOp>,
}

pub struct IndexWriter {
    operation_sender: Sender<IndexingOp>,
}

impl IndexWriter {
    pub fn insert_doc(&self, document: Document) {
        self.operation_sender
            .send(IndexingOp::Insert(vec![document]))
            .unwrap()
    }

    pub fn insert_docs(&self, documents: Vec<Document>) {
        self.operation_sender
            .send(IndexingOp::Insert(documents))
            .unwrap();
    }

    /// Commits the current in-memory segment & persist to disk.
    /// The `wait` param denotes whether you want wait for the commit
    /// operation to complete or you don't care.
    /// - Waiting (true): means committed docs will be available in
    /// search immediately after this function returns.
    /// - No Waiting (false): means committed docs will be available in
    /// search eventually. This is useful for setups favoring high ingestion.
    pub fn commit(&self, wait: bool) -> FstResult<()> {
        if !wait {
            self.operation_sender
                .send(IndexingOp::Commit(None))
                .unwrap();
            return Ok(());
        }

        let (tx, rx) = oneshot::channel();
        self.operation_sender
            .send(IndexingOp::Commit(Some(tx)))
            .unwrap();
        rx.recv().unwrap()
    }
}

enum IndexingOp {
    Insert(Vec<Document>),
    Commit(Option<CommitReplySender>),
    Shutdown(bool),
}

fn indexing_task(
    config: Arc<Config>,
    segments: Arc<RwLock<Vec<Arc<Segment>>>>,
    document_receiver: Receiver<IndexingOp>,
) -> FstResult<()> {
    let mut current_segment: Option<WritableSegment> = None;
    // starts a workers
    let segment_finalizer = SegmentFinalizer::start(&config.directory, segments);
    loop {
        let command = document_receiver
            .recv()
            .map_err(|err| FtsError::Other("failed to receive command.".to_string()))?;
        match command {
            IndexingOp::Insert(documents) => {
                if current_segment.is_none() {
                    current_segment = Some(Segment::create());
                }
                current_segment.as_mut().unwrap().insert(documents);
            }
            IndexingOp::Commit(commit_reply_sender_opt) => {
                if let Some(writable_segment) = current_segment.replace(Segment::create()) {
                    segment_finalizer.finalize(writable_segment, commit_reply_sender_opt);
                }
            }
            IndexingOp::Shutdown(commit) => {
                println!("process shutdown with commit={commit}");
                if commit {
                    if let Some(writable_segment) = current_segment.replace(Segment::create()) {
                        // No need to wait for the segment finalization.
                        // Stopping the finalizer will wait for a clean shutdown.
                        segment_finalizer.finalize(writable_segment, None);
                    }
                }
                segment_finalizer.stop()?;
                break;
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct IndexReader {
    segment_readers: Vec<SegmentReader>,
}

impl IndexReader {
    /// Returns all terms matching this range query.
    pub fn terms_range(
        &self,
        from_opt: Option<&str>,
        to_opt: Option<&str>,
    ) -> FstResult<Vec<String>> {
        //TODO: Think performance later.
        let from = from_opt.unwrap_or("\u{0000}");
        let to = to_opt.unwrap_or("\u{fff0}");

        let mut terms_set = BTreeSet::new();
        for segment_reader in self.segment_readers.iter() {
            let mut terms: BTreeSet<String> = segment_reader
                .list_terms_in_range(from, to)?
                .into_iter()
                .collect();
            terms_set.append(&mut terms);
        }
        Ok(terms_set.into_iter().collect())
    }

    /// Returns all terms matching this query.
    pub fn terms(&self, query: Query) -> FstResult<Vec<String>> {
        let mut terms_set = BTreeSet::new();
        for segment_reader in self.segment_readers.iter() {
            let mut terms: BTreeSet<String> =
                segment_reader.list_terms(&query)?.into_iter().collect();
            terms_set.append(&mut terms);
        }
        Ok(terms_set.into_iter().collect())
    }

    /// Returns the doc ids matching this query.
    pub fn query(&self, query: Query) -> FstResult<Vec<u64>> {
        let mut terms_set = BTreeSet::new();
        for segment_reader in self.segment_readers.iter() {
            let mut terms: BTreeSet<DocId> = segment_reader.search(&query)?.into_iter().collect();
            terms_set.append(&mut terms);
        }
        Ok(terms_set.into_iter().collect())
    }

    /// Returns the content of a document  with the provided id.
    pub fn fetch_doc(&self, id: DocId) -> FstResult<Bytes> {
        // Iterating in reverse is important to make doc update overshadow its old content.
        for segment_reader in self.segment_readers.iter().rev() {
            if let Some(data) = segment_reader.fetch_doc(id)? {
                return Ok(data);
            }
        }
        Err(FtsError::DocNotFound)
    }
}
