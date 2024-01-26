// use std::collections::HashMap;

use std::{
    fs::OpenOptions,
    io::{self, Seek, Write},
    path::PathBuf,
};

use bytes::Bytes;
use hashbrown::HashMap;
use memmap2::Mmap;

use crate::{
    error::FstResult, segment_component_file, DocId,
    SegmentComponent,
};

const ZSTD_LEVEL: i32 = 2;
const U64_NUM_BYTE: usize = 8;

#[derive(Debug, Clone)]
pub struct DocStoreInfo {
    pub offset: usize,
    pub length: usize,
}

/// An mmaped file backed storage for documents.
/// document can be compressed and should remain transparent to higher level.
#[derive(Debug)]
pub struct DocStore {
    index: HashMap<DocId, DocStoreInfo>,
    store: Mmap,
}

impl DocStore {
    pub fn new(
        index_directory: &PathBuf,
        segment_id: &str,
        documents: HashMap<DocId, Bytes>,
    ) -> FstResult<Self> {
        let store_file_name =
            segment_component_file(index_directory, segment_id, SegmentComponent::DocStore);
        let store_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(store_file_name)?;

        let mut store_file_writer = io::BufWriter::new(&store_file);

        let mut index = HashMap::new();
        let mut offset: usize = 0;
        for (doc_id, doc_content) in documents {
            let compressed_doc_content =
                zstd::stream::encode_all(doc_content.as_ref(), ZSTD_LEVEL)?;
            index.insert(
                doc_id,
                DocStoreInfo {
                    offset: offset,
                    length: compressed_doc_content.len(),
                },
            );

            store_file_writer.write_all(&doc_id.to_le_bytes())?;
            store_file_writer.write_all(&(compressed_doc_content.len() as u64).to_le_bytes())?;
            store_file_writer.write_all(&compressed_doc_content)?;
            offset += (2*U64_NUM_BYTE) + compressed_doc_content.len();
        }
        store_file_writer.flush()?;

        let store = unsafe { Mmap::map(&store_file)? };
        Ok(Self { index, store })
    }

    pub fn open(index_directory: &PathBuf, segment_id: &str) -> FstResult<Self> {
        let store_file_name =
            segment_component_file(index_directory, segment_id, SegmentComponent::DocStore);
        let store_file = OpenOptions::new()
            .read(true)
            .open(store_file_name)?;
        let mut store_file_reader = io::BufReader::new(&store_file);
        let mut index = HashMap::new();
        let mut offset = 0u64;
        let file_size = store_file.metadata()?.len();
        use byteorder::{LittleEndian, ReadBytesExt};
        loop {
            if offset >= file_size {
                break;
            }
            store_file_reader.seek(io::SeekFrom::Start(offset))?;
            let doc_id = store_file_reader.read_u64::<LittleEndian>()?;
            let doc_length = store_file_reader.read_u64::<LittleEndian>()?;
            
            index.insert(doc_id, DocStoreInfo{
                offset: offset as usize,
                length: doc_length as usize,
            });
            offset = offset + (2 * U64_NUM_BYTE as u64) + doc_length;
        }

        let store = unsafe { Mmap::map(&store_file)? };
        Ok(Self {
            index,
            store,
        })
    }

    pub fn fetch_doc(&self, id: DocId) -> FstResult<Option<Bytes>> {
        let Some(info) = self.index.get(&id) else {
            return Ok(None);
        };
        let offset = info.offset + (2 * U64_NUM_BYTE);
        let doc_content =
            zstd::stream::decode_all(&self.store[offset..offset + info.length])?;
        Ok(Some(Bytes::from(doc_content)))
    }
}
