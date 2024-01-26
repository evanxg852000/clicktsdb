pub mod core;
pub mod doc_store;
pub mod error;
pub mod matcher;
pub mod postings;
pub mod query;
pub mod segment;

pub use core::*;
pub use error::{FstResult, FtsError};

use std::vec;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempdir::TempDir;

    use crate::{error::FstResult, query::Query, Config, Document, Index};

    #[test]
    fn usage() -> FstResult<()> {
        let tmp_dir = TempDir::new("./data").unwrap();
        let config = Config::new(tmp_dir.path());

        // let tmp_dir = PathBuf::from("./data");
        // let config = Config::new(tmp_dir.as_path());

        let index = Index::open(config)?;

        {
            // write
            let writer = index.writer();
            writer.insert_doc(Document::new(1, "foo bar", &["foo", "bar"]));
            writer.insert_doc(Document::new(2, "foo baz", &["foo", "baz"]));
            writer.insert_doc(Document::new(3, "biz buz", &["biz", "buz"]));
            writer.commit(true)?;
        }

        {
            // read doc_store
            let doc_id = 2u64;
            let reader = index.reader();
            let data = reader.fetch_doc(doc_id).unwrap();
            let text = String::from_utf8(data.to_vec()).unwrap();
            assert_eq!(&text, "foo baz");
        }

        {
            // overwrite doc 3
            let writer = index.writer();
            writer.insert_doc(Document::new(3, "overwrite", &["biz", "buz"]));
            writer.commit(true)?;

            let reader = index.reader();
            let data = reader.fetch_doc(3).unwrap();
            let text = String::from_utf8(data.to_vec()).unwrap();
            assert_eq!(&text, "overwrite");
        }

        {
            // all terms
            let reader = index.reader();
            let terms = reader.terms(Query::All)?;
            assert_eq!(&terms, &["bar", "baz", "biz", "buz", "foo"]);
        }

        {
            // terms range query
            let reader = index.reader();
            let terms = reader.terms_range(None, None)?;
            assert_eq!(&terms, &["bar", "baz", "biz", "buz", "foo"]);

            let terms = reader.terms_range(Some("biz"), None)?;
            assert_eq!(&terms, &["biz", "buz", "foo"]);

            let terms = reader.terms_range(None, Some("buz"))?;
            assert_eq!(&terms, &["bar", "baz", "biz"]);
        }

        // { // search
        //     let reader = index.reader();
        //     let doc_ids = reader.query("foo AND bar")?;
        //     for doc_id in doc_ids {
        //         let content = reader.fetch_doc(doc_id).unwrap();
        //         let text = String::from_utf8(content.to_vec()).unwrap();
        //         println!("Found doc: {} -> {}", doc_id, text);
        //     }

        //     assert_eq!(&doc_ids, &[1]);
        // }

        index.close(false).unwrap();
        Ok(())
    }
}
